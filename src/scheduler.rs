use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

use crate::cron::CronSchedule;
use crate::errors::SchedulerError;
use crate::executor::{run_loop, StopSignal};
use crate::interval::parse_interval;
use crate::job::{Job, Schedule};
use crate::registry::JobRegistry;

/// Returned by `every()`/`cron()` when called without a `callback`, i.e.
/// when used as a decorator (`@scheduler.every("5s")` or
/// `@scheduler.cron("0 * * * *")`). Python applies a decorator by calling it
/// with the decorated function, which is exactly what `__call__` here does:
/// register the job, then hand the original function back unchanged so it
/// keeps working as a normal, directly-callable function.
#[pyclass(module = "rust_py_scheduler")]
pub struct JobDecorator {
    registry: JobRegistry,
    schedule: Schedule,
    max_retries: u32,
}

#[pymethods]
impl JobDecorator {
    fn __call__<'py>(&self, callback: Bound<'py, PyAny>) -> Bound<'py, PyAny> {
        let name = callback
            .getattr("__name__")
            .and_then(|attr| attr.extract::<String>())
            .unwrap_or_else(|_| "job".to_string());

        let job = Job::new(name, self.schedule.clone()).with_max_retries(self.max_retries);
        self.registry.insert(job, callback.clone().unbind());

        callback
    }
}

#[pyclass(module = "rust_py_scheduler")]
pub struct Scheduler {
    registry: JobRegistry,
    stop_signal: Arc<StopSignal>,
    background_thread: Mutex<Option<JoinHandle<()>>>,
}

impl Scheduler {
    /// Shared by `every()` and `cron()`: either registers the job
    /// immediately (returning its id as a `str`) when a `callback` is given,
    /// or returns a `JobDecorator` that registers the function it's applied
    /// to. Keeps the two public methods down to "parse a schedule, then
    /// register it".
    fn register(
        &self,
        py: Python<'_>,
        schedule: Schedule,
        callback: Option<Bound<'_, PyAny>>,
        max_retries: u32,
    ) -> PyResult<Py<PyAny>> {
        match callback {
            Some(callback) => {
                let name = callback
                    .getattr("__name__")
                    .and_then(|attr| attr.extract::<String>())
                    .unwrap_or_else(|_| "job".to_string());

                let job = Job::new(name, schedule).with_max_retries(max_retries);
                let job_id = self.registry.insert(job, callback.unbind());

                Ok(PyString::new(py, &job_id).into_any().unbind())
            }
            None => {
                let decorator = JobDecorator {
                    registry: self.registry.clone(),
                    schedule,
                    max_retries,
                };
                Ok(Py::new(py, decorator)?.into_any())
            }
        }
    }
}

#[pymethods]
impl Scheduler {
    #[new]
    fn new() -> Self {
        Scheduler {
            registry: JobRegistry::new(),
            stop_signal: Arc::new(StopSignal::new()),
            background_thread: Mutex::new(None),
        }
    }

    /// Two calling conventions:
    /// - `scheduler.every("5s", hello)`: registers immediately, returns the job id (str).
    /// - `@scheduler.every("5s")`: returns an `EveryDecorator`, which registers
    ///   the job once Python applies it to the decorated function.
    ///
    /// `max_retries` (default 0): extra attempts on failure before a tick
    /// counts as an error. Works the same in either calling convention.
    #[pyo3(signature = (interval, callback=None, max_retries=0))]
    fn every(
        &self,
        py: Python<'_>,
        interval: &str,
        callback: Option<Bound<'_, PyAny>>,
        max_retries: u32,
    ) -> PyResult<Py<PyAny>> {
        let schedule = Schedule::Every(parse_interval(interval)?);
        Ok(self.register(py, schedule, callback, max_retries)?)
    }

    /// Schedules a job from a 5-field Unix cron expression
    /// (`minute hour day-of-month month day-of-week`), e.g.
    /// `scheduler.cron("0 9 * * 1-5", fn)` for "weekdays at 9am". Same two
    /// calling conventions as `every()`: a direct call returns the job id,
    /// and `@scheduler.cron("0 * * * *")` returns a decorator. Times are
    /// evaluated in the system's local timezone. An invalid expression
    /// raises `ValueError` immediately, at registration time.
    #[pyo3(signature = (expression, callback=None, max_retries=0))]
    fn cron(
        &self,
        py: Python<'_>,
        expression: &str,
        callback: Option<Bound<'_, PyAny>>,
        max_retries: u32,
    ) -> PyResult<Py<PyAny>> {
        let schedule = Schedule::Cron(CronSchedule::parse(expression)?);
        Ok(self.register(py, schedule, callback, max_retries)?)
    }

    /// A snapshot of every registered job's current state, as a list of
    /// dicts with keys: id, name, schedule, enabled, run_count, error_count,
    /// last_run_at, next_run_at, max_retries, last_error. Plain dicts
    /// (rather than a dedicated pyclass) keep this easy to
    /// print/inspect/serialize from Python.
    fn list_jobs(&self, py: Python<'_>) -> PyResult<Vec<Py<PyDict>>> {
        self.registry
            .snapshot()
            .into_iter()
            .map(|job| {
                let dict = PyDict::new(py);
                dict.set_item("id", job.id)?;
                dict.set_item("name", job.name)?;
                dict.set_item("schedule", job.schedule.describe())?;
                dict.set_item("enabled", job.enabled)?;
                dict.set_item("run_count", job.run_count)?;
                dict.set_item("error_count", job.error_count)?;
                dict.set_item("last_run_at", job.last_run_at)?;
                dict.set_item("next_run_at", job.next_run_at)?;
                dict.set_item("max_retries", job.max_retries)?;
                dict.set_item("last_error", job.last_error)?;
                Ok(dict.unbind())
            })
            .collect()
    }

    /// Unregisters a job by id, so it never runs again. Raises `KeyError`
    /// if no job with that id exists (mirrors `dict.pop()`'s semantics on a
    /// missing key, which is the closest Python analogue).
    fn remove_job(&self, job_id: &str) -> Result<(), SchedulerError> {
        if self.registry.remove(job_id) {
            Ok(())
        } else {
            Err(SchedulerError::JobNotFound(job_id.to_string()))
        }
    }

    /// Blocks the calling thread, executing due jobs until `shutdown()` is
    /// called. Releases the GIL while idle/sleeping so other Python threads
    /// (e.g. one calling `shutdown()`) can keep running.
    fn run(&self, py: Python<'_>) {
        py.detach(|| {
            run_loop(&self.registry, &self.stop_signal);
        });
    }

    /// Spawns the same loop used by `run()` on a dedicated OS thread and
    /// returns immediately, so the caller (e.g. FastAPI's event loop) never
    /// blocks.
    fn start_background(&self) -> Result<(), SchedulerError> {
        let mut guard = self
            .background_thread
            .lock()
            .expect("background thread mutex poisoned");

        if guard.is_some() {
            return Err(SchedulerError::AlreadyRunningInBackground);
        }

        let registry = self.registry.clone();
        let stop_signal = Arc::clone(&self.stop_signal);

        let handle = thread::Builder::new()
            .name("rust-py-scheduler".to_string())
            .spawn(move || run_loop(&registry, &stop_signal))
            .map_err(|err| SchedulerError::BackgroundThreadSpawnFailed(err.to_string()))?;

        *guard = Some(handle);
        Ok(())
    }

    /// Signals a running `run()`/background loop to stop, then (if running
    /// in background) waits for that thread to actually finish. Safe to
    /// call from any thread, including from inside a job callback.
    fn shutdown(&self, py: Python<'_>) {
        self.stop_signal.stop();

        let handle = {
            let mut guard = self
                .background_thread
                .lock()
                .expect("background thread mutex poisoned");
            guard.take()
        };

        if let Some(handle) = handle {
            py.detach(|| {
                let _ = handle.join();
            });
        }
    }
}

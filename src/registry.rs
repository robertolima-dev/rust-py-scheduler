use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Local;
use pyo3::prelude::*;

use crate::cron;
use crate::job::{Job, Schedule};
use crate::time_utils;

pub struct RegisteredJob {
    pub job: Job,
    pub callback: Py<PyAny>,
    /// Monotonic deadline used internally by the run loop. Kept separate
    /// from `Job::next_run_at` (a `String`, meant for Python-facing display).
    pub next_run_at: Instant,
}

#[derive(Clone, Default)]
pub struct JobRegistry {
    jobs: Arc<Mutex<HashMap<String, RegisteredJob>>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, mut job: Job, callback: Py<PyAny>) -> String {
        let id = job.id.clone();
        let (next_run_at, next_display) = Self::schedule_next(&job.schedule);
        job.next_run_at = next_display;

        let mut jobs = self.jobs.lock().expect("job registry mutex poisoned");
        jobs.insert(
            id.clone(),
            RegisteredJob {
                job,
                callback,
                next_run_at,
            },
        );
        id
    }

    /// Removes a job by id. Returns whether it was actually there. Safe to
    /// call while the run loop is mid-iteration over due jobs: `run_job`
    /// re-fetches the job by id after releasing the lock to call the
    /// callback, so a concurrent removal just makes that lookup miss.
    pub fn remove(&self, job_id: &str) -> bool {
        let mut jobs = self.jobs.lock().expect("job registry mutex poisoned");
        jobs.remove(job_id).is_some()
    }

    /// A point-in-time copy of every registered job, sorted by id so the
    /// order is stable/deterministic for callers (and tests).
    pub fn snapshot(&self) -> Vec<Job> {
        let jobs = self.jobs.lock().expect("job registry mutex poisoned");
        let mut snapshot: Vec<Job> = jobs.values().map(|registered| registered.job.clone()).collect();
        snapshot.sort_by(|a, b| a.id.cmp(&b.id));
        snapshot
    }

    /// Ids of every enabled job whose deadline has already passed.
    pub fn due_job_ids(&self, now: Instant) -> Vec<String> {
        let jobs = self.jobs.lock().expect("job registry mutex poisoned");
        jobs.iter()
            .filter(|(_, registered)| registered.job.enabled && registered.next_run_at <= now)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Earliest upcoming deadline among enabled jobs, used by the run loop
    /// to know exactly how long it can sleep for.
    pub fn next_deadline(&self) -> Option<Instant> {
        let jobs = self.jobs.lock().expect("job registry mutex poisoned");
        jobs.values()
            .filter(|registered| registered.job.enabled)
            .map(|registered| registered.next_run_at)
            .min()
    }

    /// Calls the job's Python callback under the GIL, then reschedules it.
    /// On failure, retries immediately (no backoff) up to `max_retries`
    /// additional times before giving up: a tick only counts as an error
    /// once every attempt for it has failed. Exceptions are always printed
    /// (one per attempt) but never propagated: one failing job must not
    /// stop the others. Note this is run outside any lock on `self.jobs`,
    /// so it's safe for the callback to call back into e.g. `remove_job()`
    /// on the same scheduler without deadlocking.
    pub fn run_job(&self, job_id: &str) {
        let (callback, max_retries) = Python::attach(|py| {
            let jobs = self.jobs.lock().expect("job registry mutex poisoned");
            match jobs.get(job_id) {
                Some(registered) => (
                    Some(registered.callback.clone_ref(py)),
                    registered.job.max_retries,
                ),
                None => (None, 0),
            }
        });

        let Some(callback) = callback else {
            return;
        };

        let mut last_error: Option<String> = None;

        for _attempt in 0..=max_retries {
            let outcome = Python::attach(|py| match callback.call0(py) {
                Ok(_) => None,
                Err(err) => {
                    let message = err.to_string();
                    err.print(py);
                    Some(message)
                }
            });

            match outcome {
                None => {
                    last_error = None;
                    break;
                }
                Some(message) => last_error = Some(message),
            }
        }

        let succeeded = last_error.is_none();

        let mut jobs = self.jobs.lock().expect("job registry mutex poisoned");
        if let Some(registered) = jobs.get_mut(job_id) {
            if succeeded {
                registered.job.run_count += 1;
            } else {
                registered.job.error_count += 1;
            }
            registered.job.last_error = last_error;
            let (next_run_at, next_display) = Self::schedule_next(&registered.job.schedule);
            registered.next_run_at = next_run_at;
            registered.job.last_run_at = Some(time_utils::now_as_string());
            registered.job.next_run_at = next_display;
        }
    }

    /// The next monotonic deadline for a schedule, plus the matching
    /// Unix-timestamp string for `Job::next_run_at` (display only). Interval
    /// jobs add a fixed `Duration` to now; cron jobs compute the next
    /// wall-clock occurrence and convert the gap into an `Instant` (see
    /// `cron::next_deadline`). Both are anchored to the *same* "now" so the
    /// monotonic deadline and the displayed timestamp agree.
    fn schedule_next(schedule: &Schedule) -> (Instant, Option<String>) {
        match schedule {
            Schedule::Every(duration) => (
                Instant::now() + *duration,
                Some(time_utils::future_as_string(duration.as_secs())),
            ),
            Schedule::Cron(cron_schedule) => {
                cron::next_deadline(cron_schedule, Instant::now(), Local::now())
            }
        }
    }
}

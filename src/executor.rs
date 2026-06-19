use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::registry::JobRegistry;

/// How long the loop sleeps when there are no jobs at all, just so it can
/// notice new jobs (or a stop request) without spinning the CPU.
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Lets `shutdown()` wake up a thread that is sleeping inside `run()`,
/// instead of that thread polling a flag in a busy loop.
pub struct StopSignal {
    stopped: Mutex<bool>,
    condvar: Condvar,
}

impl StopSignal {
    pub fn new() -> Self {
        StopSignal {
            stopped: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    pub fn stop(&self) {
        let mut stopped = self.stopped.lock().expect("stop signal mutex poisoned");
        *stopped = true;
        self.condvar.notify_all();
    }

    pub fn is_stopped(&self) -> bool {
        *self.stopped.lock().expect("stop signal mutex poisoned")
    }

    /// Sleeps until `deadline`, unless `stop()` is called first, in which
    /// case it returns immediately.
    pub fn wait_until(&self, deadline: Instant) {
        let mut guard = self.stopped.lock().expect("stop signal mutex poisoned");
        loop {
            if *guard {
                return;
            }
            let now = Instant::now();
            if now >= deadline {
                return;
            }
            let (new_guard, result) = self
                .condvar
                .wait_timeout(guard, deadline - now)
                .expect("stop signal mutex poisoned");
            guard = new_guard;
            if result.timed_out() || *guard {
                return;
            }
        }
    }
}

/// The blocking scheduling loop shared by `run()` and (later) `start_background()`.
pub fn run_loop(registry: &JobRegistry, stop_signal: &StopSignal) {
    loop {
        if stop_signal.is_stopped() {
            return;
        }

        for job_id in registry.due_job_ids(Instant::now()) {
            if stop_signal.is_stopped() {
                return;
            }
            registry.run_job(&job_id);
        }

        let deadline = registry
            .next_deadline()
            .unwrap_or_else(|| Instant::now() + IDLE_POLL_INTERVAL);
        stop_signal.wait_until(deadline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn wait_until_returns_early_when_stopped() {
        let signal = Arc::new(StopSignal::new());
        let signal_clone = Arc::clone(&signal);

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            signal_clone.stop();
        });

        let started = Instant::now();
        signal.wait_until(started + Duration::from_secs(5));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn wait_until_respects_deadline_when_never_stopped() {
        let signal = StopSignal::new();
        let started = Instant::now();
        signal.wait_until(started + Duration::from_millis(30));
        assert!(started.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn is_stopped_reflects_stop_calls() {
        let signal = StopSignal::new();
        assert!(!signal.is_stopped());
        signal.stop();
        assert!(signal.is_stopped());
    }
}

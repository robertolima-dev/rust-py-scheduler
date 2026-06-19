use std::time::Duration;

use crate::cron::CronSchedule;

#[derive(Debug, Clone)]
pub enum Schedule {
    Every(Duration),
    Cron(CronSchedule),
}

impl Schedule {
    /// Human-readable form used by `Scheduler.list_jobs()`. For intervals the
    /// original "5s"/"2m"/"1h" string isn't kept around after parsing, so
    /// this reconstructs a (always-in-seconds) description from the
    /// `Duration`; cron jobs report their original expression verbatim.
    pub fn describe(&self) -> String {
        match self {
            Schedule::Every(duration) => format!("every {}s", duration.as_secs()),
            Schedule::Cron(cron) => format!("cron {}", cron.expression()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub schedule: Schedule,
    pub enabled: bool,
    pub run_count: u64,
    pub error_count: u64,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    /// Extra attempts allowed after the first failure, before a tick is
    /// counted as an error. 0 (the default) means "no retries".
    pub max_retries: u32,
    /// Message from the most recent failed attempt. Cleared back to `None`
    /// the next time the job runs successfully.
    pub last_error: Option<String>,
}

impl Job {
    pub fn new(name: String, schedule: Schedule) -> Self {
        Job {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            schedule,
            enabled: true,
            run_count: 0,
            error_count: 0,
            last_run_at: None,
            next_run_at: None,
            max_retries: 0,
            last_error: None,
        }
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_job_has_sane_defaults() {
        let job = Job::new("cleanup".to_string(), Schedule::Every(Duration::from_secs(5)));
        assert_eq!(job.name, "cleanup");
        assert!(job.enabled);
        assert_eq!(job.run_count, 0);
        assert_eq!(job.error_count, 0);
        assert!(job.last_run_at.is_none());
        assert!(job.next_run_at.is_none());
        assert_eq!(job.max_retries, 0);
        assert!(job.last_error.is_none());
    }

    #[test]
    fn with_max_retries_overrides_the_default() {
        let job = Job::new("cleanup".to_string(), Schedule::Every(Duration::from_secs(5)))
            .with_max_retries(3);
        assert_eq!(job.max_retries, 3);
    }

    #[test]
    fn each_job_gets_a_unique_id() {
        let job_a = Job::new("a".to_string(), Schedule::Every(Duration::from_secs(1)));
        let job_b = Job::new("b".to_string(), Schedule::Every(Duration::from_secs(1)));
        assert_ne!(job_a.id, job_b.id);
    }
}

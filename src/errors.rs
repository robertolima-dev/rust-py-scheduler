use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::PyErr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("invalid interval '{0}': expected formats like '10s', '5m' or '1h'")]
    InvalidInterval(String),

    #[error("invalid cron expression '{0}': expected 5 fields like '0 * * * *' (minute hour day-of-month month day-of-week)")]
    InvalidCron(String),

    #[error("scheduler is already running in the background")]
    AlreadyRunningInBackground,

    #[error("failed to start background thread: {0}")]
    BackgroundThreadSpawnFailed(String),

    #[error("no job registered with id '{0}'")]
    JobNotFound(String),
}

impl From<SchedulerError> for PyErr {
    fn from(err: SchedulerError) -> PyErr {
        match err {
            SchedulerError::InvalidInterval(_) | SchedulerError::InvalidCron(_) => {
                PyValueError::new_err(err.to_string())
            }
            SchedulerError::AlreadyRunningInBackground
            | SchedulerError::BackgroundThreadSpawnFailed(_) => {
                PyRuntimeError::new_err(err.to_string())
            }
            SchedulerError::JobNotFound(_) => PyKeyError::new_err(err.to_string()),
        }
    }
}

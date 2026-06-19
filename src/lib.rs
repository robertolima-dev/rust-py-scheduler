use pyo3::prelude::*;

mod cron;
mod errors;
mod executor;
mod interval;
mod job;
mod registry;
mod scheduler;
mod time_utils;

#[pymodule]
mod rust_py_scheduler {
    #[pymodule_export]
    use crate::scheduler::Scheduler;
    #[pymodule_export]
    use crate::scheduler::JobDecorator;
}

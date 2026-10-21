use std::sync::Arc;
use tokio::sync::Mutex;

use crate::app_data::AppData;
use crate::scheduler::Scheduler;

/// Lock ordering: when both locks are needed, always lock `app_data` first, then `scheduler`.
/// Taking them in the opposite order anywhere can deadlock against the commands that add,
/// update or delete jobs.
#[derive(Clone)]
pub struct AppState {
    pub app_data: Arc<Mutex<AppData>>,
    pub scheduler: Arc<Mutex<Scheduler>>,
}

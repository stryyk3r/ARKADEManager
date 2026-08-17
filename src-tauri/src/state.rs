use std::sync::Arc;
use tokio::sync::Mutex;

use crate::app_data::AppData;
use crate::scheduler::Scheduler;

#[derive(Clone)]
pub struct AppState {
    pub app_data: Arc<Mutex<AppData>>,
    pub scheduler: Arc<Mutex<Scheduler>>,
}

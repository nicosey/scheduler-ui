use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum JobSource {
    Cron,
    Launchd { label: String, plist_path: String },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: usize,
    pub name: String,
    pub schedule: String,
    pub command: String,
    pub enabled: bool,
    pub source: JobSource,
    pub last_run: Option<DateTime<Utc>>,
    pub last_status: Option<RunStatus>,
    pub last_duration: Option<String>,
    pub logs: Vec<RunLog>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum RunStatus {
    Success,
    Error,
    Running,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct RunLog {
    pub timestamp: DateTime<Utc>,
    pub status: RunStatus,
    pub duration: String,
    pub output: String,
}

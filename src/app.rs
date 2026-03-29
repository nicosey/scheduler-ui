use std::collections::HashSet;

use crate::cron::write_crontab;
use crate::launchd::{launchctl_load, launchctl_unload};
use crate::models::{CronJob, JobSource};

pub struct SchedulerApp {
    pub jobs: Vec<CronJob>,
    pub crontab_preamble: Vec<String>,
    pub use_mock: bool,
    pub show_add_dialog: bool,
    pub new_name: String,
    pub new_schedule: String,
    pub new_command: String,
    pub new_label: String,
    pub new_source_is_launchd: bool,
    pub status_msg: Option<String>,
    pub expanded_jobs: HashSet<usize>,
}

impl SchedulerApp {
    pub fn new(jobs: Vec<CronJob>, crontab_preamble: Vec<String>, use_mock: bool) -> Self {
        Self {
            jobs,
            crontab_preamble,
            use_mock,
            show_add_dialog: false,
            new_name: String::new(),
            new_schedule: String::new(),
            new_command: String::new(),
            new_label: String::new(),
            new_source_is_launchd: false,
            status_msg: None,
            expanded_jobs: HashSet::new(),
        }
    }

    pub fn save_crontab(&mut self) {
        if self.use_mock { return; }
        let cron_jobs: Vec<CronJob> = self.jobs.iter()
            .filter(|j| j.source == JobSource::Cron)
            .cloned()
            .collect();
        match write_crontab(&self.crontab_preamble, &cron_jobs) {
            Ok(_)  => self.status_msg = Some("Crontab saved.".into()),
            Err(e) => self.status_msg = Some(format!("Error: {e}")),
        }
    }

    pub fn toggle_launchd(&mut self, id: usize) {
        let Some(job) = self.jobs.iter().find(|j| j.id == id) else { return };
        let JobSource::Launchd { plist_path, .. } = job.source.clone() else { return };
        let currently_enabled = job.enabled;

        let result = if currently_enabled {
            launchctl_unload(&plist_path).map(|_| false)
        } else {
            launchctl_load(&plist_path).map(|_| true)
        };

        match result {
            Ok(new_state) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.enabled = new_state;
                }
                self.status_msg = Some(if new_state { "Job loaded." } else { "Job unloaded." }.into());
            }
            Err(e) => self.status_msg = Some(format!("Error: {e}")),
        }
    }

    pub fn clear_form(&mut self) {
        self.new_name.clear();
        self.new_schedule.clear();
        self.new_command.clear();
        self.new_label.clear();
        self.new_source_is_launchd = false;
    }
}

mod models;
mod cron;
mod launchd;
mod util;
mod theme;
mod app;
mod ui;

use eframe::egui;

use app::SchedulerApp;
use cron::parse_crontab;
use launchd::parse_launchd_agents;
use util::sample_jobs;

fn main() -> eframe::Result<()> {
    let use_mock = std::env::args().any(|a| a == "--mock");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 620.0])
            .with_title("Scheduler UI"),
        ..Default::default()
    };

    eframe::run_native(
        "Scheduler UI",
        options,
        Box::new(move |_cc| {
            let app = if use_mock {
                SchedulerApp::new(sample_jobs(), Vec::new(), true)
            } else {
                let (mut jobs, preamble) = parse_crontab();
                let launchd = parse_launchd_agents(jobs.len());
                jobs.extend(launchd);
                let jobs = if jobs.is_empty() { sample_jobs() } else { jobs };
                SchedulerApp::new(jobs, preamble, false)
            };
            Ok(Box::new(app))
        }),
    )
}

use std::process::Command;

use chrono::Local;
use eframe::egui;

use crate::app::SchedulerApp;
use crate::launchd::delete_launchd_job;
use crate::models::{JobSource, RunLog, RunStatus};
use crate::theme::Theme;
use crate::util::{human_schedule, time_ago};

impl SchedulerApp {
    pub fn render_job_list(&mut self, ui: &mut egui::Ui) {
        let mut toggle_id = None;
        let mut run_id    = None;
        let mut delete_id = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for job in &self.jobs {
                let collapse_id = ui.make_persistent_id(job.id);
                let mut collapse = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(), collapse_id, false,
                );
                let is_open = collapse.is_open();
                let mut toggle_expand = false;

                let bg           = if is_open { Theme::SELECTED } else { Theme::PANEL };
                let stroke_color = if is_open { Theme::ACCENT } else { Theme::BORDER };

                egui::Frame::new()
                    .fill(bg)
                    .stroke(egui::Stroke::new(1.0, stroke_color))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let status_color = if !job.enabled {
                                Theme::TEXT_DIM
                            } else {
                                match &job.last_status {
                                    Some(RunStatus::Success) => Theme::SUCCESS,
                                    Some(RunStatus::Error)   => Theme::ERROR,
                                    Some(RunStatus::Running) => Theme::ACCENT,
                                    None => Theme::TEXT_DIM,
                                }
                            };
                            let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot_rect.center(), 4.0, status_color);

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&job.name)
                                        .size(14.0).strong().color(egui::Color32::from_rgb(241, 245, 249)));
                                    if !job.enabled {
                                        ui.label(egui::RichText::new("PAUSED").size(9.0).color(Theme::TEXT_DIM));
                                    }
                                    let (badge, badge_color) = match &job.source {
                                        JobSource::Cron           => ("CRON",     Theme::TEXT_DIM),
                                        JobSource::Launchd { .. } => ("LAUNCHD", Theme::LAUNCHD),
                                    };
                                    ui.label(egui::RichText::new(badge).size(9.0).color(badge_color));
                                });
                                ui.label(egui::RichText::new(human_schedule(&job.schedule))
                                    .size(11.0).color(Theme::TEXT_DIM));
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let expand_icon = if is_open { "▾" } else { "▸" };
                                if ui.add(egui::Button::new(egui::RichText::new(expand_icon).size(14.0).color(Theme::TEXT_DIM))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text("Show details").clicked() {
                                    toggle_expand = true;
                                }

                                if ui.add(egui::Button::new(egui::RichText::new("✕").size(12.0).strong().color(egui::Color32::WHITE))
                                    .fill(egui::Color32::from_rgb(180, 30, 30))
                                    .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 50, 50)))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text("Delete job").clicked() {
                                    delete_id = Some(job.id);
                                }

                                let (toggle_text, toggle_color, toggle_tip) = if job.enabled {
                                    ("◉", Theme::SUCCESS, "Disable job")
                                } else {
                                    ("○", Theme::TEXT_DIM, "Enable job")
                                };
                                if ui.add(egui::Button::new(egui::RichText::new(toggle_text).size(14.0).color(toggle_color))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text(toggle_tip).clicked() {
                                    toggle_id = Some(job.id);
                                }

                                if ui.add(egui::Button::new(egui::RichText::new("▶").size(12.0).color(Theme::TEXT_MUTED))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text("Run now").clicked() {
                                    run_id = Some(job.id);
                                }

                                ui.add_space(16.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(110.0, 36.0),
                                    egui::Layout::top_down(egui::Align::Max),
                                    |ui| {
                                        ui.label(egui::RichText::new("LAST RUN").size(9.0).color(Theme::TEXT_DIM));
                                        let last = job.last_run
                                            .map(|dt| time_ago(&dt))
                                            .unwrap_or_else(|| "—".into());
                                        let last_color = match &job.last_status {
                                            Some(RunStatus::Success) => Theme::SUCCESS,
                                            Some(RunStatus::Error)   => Theme::ERROR,
                                            _                        => Theme::TEXT_MUTED,
                                        };
                                        ui.label(egui::RichText::new(last).size(12.0).color(last_color));
                                    },
                                );
                            });
                        });

                        if is_open {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);

                            // Command block
                            egui::Frame::new()
                                .fill(Theme::BG)
                                .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(egui::RichText::new("$").size(12.0).color(Theme::TEXT_DIM));
                                        ui.label(egui::RichText::new(&job.command).size(12.0).color(Theme::TEXT_MUTED));
                                    });
                                    // Show plist path for launchd jobs
                                    if let JobSource::Launchd { plist_path, .. } = &job.source {
                                        ui.label(egui::RichText::new(plist_path).size(10.0).color(Theme::TEXT_DIM));
                                    }
                                });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("RUN HISTORY").size(10.0).color(Theme::TEXT_DIM));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let log_tip = match &job.source {
                                        JobSource::Launchd { .. } => "Open ~/Library/Logs in Finder",
                                        JobSource::Cron           => "Open Console.app",
                                    };
                                    if ui.add(egui::Button::new(
                                            egui::RichText::new("Open Logs").size(10.0).color(Theme::TEXT_DIM))
                                        .fill(egui::Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                        .corner_radius(4.0)
                                    ).on_hover_text(log_tip).clicked() {
                                        match &job.source {
                                            JobSource::Launchd { .. } => {
                                                let _ = Command::new("open")
                                                    .arg(format!("{}/Library/Logs",
                                                        std::env::var("HOME").unwrap_or_default()))
                                                    .spawn();
                                            }
                                            JobSource::Cron => {
                                                let _ = Command::new("open")
                                                    .args(["-a", "Console"])
                                                    .spawn();
                                            }
                                        }
                                    }
                                });
                            });
                            ui.add_space(4.0);

                            if job.logs.is_empty() {
                                ui.label(egui::RichText::new("No runs yet").size(12.0).color(Theme::TEXT_DIM));
                            }

                            for (i, log) in job.logs.iter().enumerate() {
                                let log_bg = if i == 0 {
                                    egui::Color32::from_rgba_premultiplied(26, 27, 35, 128)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                egui::Frame::new()
                                    .fill(log_bg)
                                    .corner_radius(4.0)
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let log_color = match log.status {
                                                RunStatus::Success => Theme::SUCCESS,
                                                RunStatus::Error   => Theme::ERROR,
                                                RunStatus::Running => Theme::ACCENT,
                                            };
                                            let (dot_r, _) = ui.allocate_exact_size(
                                                egui::vec2(8.0, 8.0), egui::Sense::hover());
                                            ui.painter().circle_filled(dot_r.center(), 3.5, log_color);

                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    let ts = log.timestamp.with_timezone(&Local)
                                                        .format("%d %b %H:%M").to_string();
                                                    ui.label(egui::RichText::new(ts).size(11.0).color(Theme::TEXT_DIM));
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(egui::RichText::new(&log.duration).size(11.0).color(Theme::TEXT_DIM));
                                                    });
                                                });
                                                let out_color = if log.status == RunStatus::Error {
                                                    egui::Color32::from_rgb(252, 165, 165)
                                                } else {
                                                    Theme::TEXT_MUTED
                                                };
                                                ui.label(egui::RichText::new(&log.output).size(12.0).color(out_color));
                                            });
                                        });
                                    });
                            }
                        }
                    });

                if toggle_expand {
                    collapse.set_open(!is_open);
                }
                collapse.store(ui.ctx());

                ui.add_space(4.0);
            }
        });

        if let Some(id) = toggle_id {
            let source = self.jobs.iter().find(|j| j.id == id).map(|j| j.source.clone());
            match source {
                Some(JobSource::Cron) => {
                    if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                        job.enabled = !job.enabled;
                    }
                    self.save_crontab();
                }
                Some(JobSource::Launchd { .. }) => self.toggle_launchd(id),
                None => {}
            }
        }

        if let Some(id) = run_id {
            if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                let start = std::time::Instant::now();
                let output = Command::new("sh").arg("-c").arg(&job.command).output();
                let elapsed = start.elapsed();
                let duration_str = if elapsed.as_secs() > 0 {
                    format!("{}s", elapsed.as_secs())
                } else {
                    format!("{}ms", elapsed.as_millis())
                };
                let (status, out_text) = match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        let text = if stdout.trim().is_empty() { stderr } else { stdout };
                        let text = text.trim().to_string();
                        if out.status.success() {
                            (RunStatus::Success, if text.is_empty() { "Completed successfully.".into() } else { text })
                        } else {
                            (RunStatus::Error, if text.is_empty() { "Command failed.".into() } else { text })
                        }
                    }
                    Err(e) => (RunStatus::Error, format!("Failed to execute: {}", e)),
                };
                let log = RunLog {
                    timestamp: chrono::Utc::now(),
                    status: status.clone(),
                    duration: duration_str.clone(),
                    output: out_text,
                };
                job.last_run = Some(chrono::Utc::now());
                job.last_status = Some(status);
                job.last_duration = Some(duration_str);
                job.logs.insert(0, log);
            }
        }

        if let Some(id) = delete_id {
            let source = self.jobs.iter().find(|j| j.id == id).map(|j| j.source.clone());
            match source {
                Some(JobSource::Cron) => {
                    self.jobs.retain(|j| j.id != id);
                    self.save_crontab();
                }
                Some(JobSource::Launchd { plist_path, .. }) => {
                    match delete_launchd_job(&plist_path) {
                        Ok(_) => {
                            self.jobs.retain(|j| j.id != id);
                            self.status_msg = Some("Launchd job deleted.".into());
                        }
                        Err(e) => self.status_msg = Some(format!("Error: {e}")),
                    }
                }
                None => {}
            }
        }
    }
}

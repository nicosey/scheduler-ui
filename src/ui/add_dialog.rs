use eframe::egui;

use crate::app::SchedulerApp;
use crate::launchd::write_launchd_plist;
use crate::models::{CronJob, JobSource};
use crate::theme::Theme;

impl SchedulerApp {
    pub fn render_add_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("New Job")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(420.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // Source selector
                ui.label(egui::RichText::new("SOURCE").size(10.0).color(Theme::TEXT_DIM));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.new_source_is_launchd, false, "Cron");
                    ui.radio_value(&mut self.new_source_is_launchd, true,  "Launchd");
                });
                ui.add_space(6.0);

                ui.label(egui::RichText::new("NAME").size(10.0).color(Theme::TEXT_DIM));
                ui.text_edit_singleline(&mut self.new_name);
                ui.add_space(6.0);

                if self.new_source_is_launchd {
                    ui.label(egui::RichText::new("LABEL").size(10.0).color(Theme::TEXT_DIM));
                    ui.text_edit_singleline(&mut self.new_label);
                    ui.label(egui::RichText::new("e.g. com.user.my-task").size(10.0).color(Theme::TEXT_DIM));
                    ui.add_space(6.0);
                }

                ui.label(egui::RichText::new("SCHEDULE").size(10.0).color(Theme::TEXT_DIM));
                ui.text_edit_singleline(&mut self.new_schedule);
                ui.label(egui::RichText::new("e.g. 0 7 * * *").size(10.0).color(Theme::TEXT_DIM));
                ui.add_space(6.0);

                ui.label(egui::RichText::new("COMMAND").size(10.0).color(Theme::TEXT_DIM));
                ui.text_edit_singleline(&mut self.new_command);
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Cancel")
                        .fill(Theme::PANEL)
                        .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                        .min_size(egui::vec2(80.0, 30.0))
                    ).clicked() {
                        self.show_add_dialog = false;
                        self.clear_form();
                    }

                    let label_ok = !self.new_source_is_launchd || !self.new_label.is_empty();
                    let can_add  = !self.new_name.is_empty()
                        && !self.new_schedule.is_empty()
                        && !self.new_command.is_empty()
                        && label_ok;

                    if ui.add(egui::Button::new(egui::RichText::new("Add Job").color(egui::Color32::WHITE))
                        .fill(if can_add { Theme::ACCENT } else { Theme::BORDER })
                        .min_size(egui::vec2(80.0, 30.0))
                    ).clicked() && can_add {
                        let next_id = self.jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;

                        if self.new_source_is_launchd {
                            match write_launchd_plist(&self.new_label, &self.new_command, &self.new_schedule) {
                                Ok(plist_path) => {
                                    self.jobs.push(CronJob {
                                        id: next_id,
                                        name: self.new_name.clone(),
                                        schedule: self.new_schedule.clone(),
                                        command: self.new_command.clone(),
                                        enabled: true,
                                        source: JobSource::Launchd {
                                            label: self.new_label.clone(),
                                            plist_path,
                                        },
                                        last_run: None,
                                        last_status: None,
                                        last_duration: None,
                                        logs: Vec::new(),
                                    });
                                    self.show_add_dialog = false;
                                    self.clear_form();
                                    self.status_msg = Some("Launchd job created.".into());
                                }
                                Err(e) => self.status_msg = Some(format!("Error: {e}")),
                            }
                        } else {
                            self.jobs.push(CronJob {
                                id: next_id,
                                name: self.new_name.clone(),
                                schedule: self.new_schedule.clone(),
                                command: self.new_command.clone(),
                                enabled: true,
                                source: JobSource::Cron,
                                last_run: None,
                                last_status: None,
                                last_duration: None,
                                logs: Vec::new(),
                            });
                            self.show_add_dialog = false;
                            self.clear_form();
                            self.save_crontab();
                        }
                    }
                });
            });

        if !open {
            self.show_add_dialog = false;
            self.clear_form();
        }
    }
}

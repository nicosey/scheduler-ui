use eframe::egui;

use crate::app::SchedulerApp;
use crate::cron::parse_crontab;
use crate::launchd::parse_launchd_agents;
use crate::theme::Theme;
use crate::util::sample_jobs;

impl SchedulerApp {
    pub fn render_header(&mut self, ui: &mut egui::Ui) {
        let active = self.jobs.iter().filter(|j| j.enabled).count();

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 8.0, Theme::ACCENT);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "⏱",
                egui::FontId::proportional(18.0), Theme::TEXT);

            ui.vertical(|ui| {
                let title = if self.use_mock { "Scheduler UI (mock)" } else { "Scheduler UI" };
                ui.label(egui::RichText::new(title).size(20.0).strong()
                    .color(egui::Color32::from_rgb(241, 245, 249)));
                ui.label(egui::RichText::new(format!("{} jobs · {} active", self.jobs.len(), active))
                    .size(11.0).color(Theme::TEXT_DIM));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new("+ New Job").size(12.0).color(egui::Color32::WHITE))
                    .fill(Theme::ACCENT).corner_radius(8.0).min_size(egui::vec2(90.0, 32.0))
                ).clicked() {
                    self.show_add_dialog = true;
                }

                if ui.add(egui::Button::new(egui::RichText::new("↻ Refresh").size(12.0).color(Theme::TEXT_MUTED))
                    .fill(Theme::PANEL).stroke(egui::Stroke::new(1.0, Theme::BORDER))
                    .corner_radius(8.0).min_size(egui::vec2(80.0, 32.0))
                ).clicked() {
                    if self.use_mock {
                        self.jobs = sample_jobs();
                    } else {
                        let (mut jobs, preamble) = parse_crontab();
                        let launchd = parse_launchd_agents(jobs.len());
                        jobs.extend(launchd);
                        self.jobs = if jobs.is_empty() { sample_jobs() } else { jobs };
                        self.crontab_preamble = preamble;
                    }
                    self.status_msg = None;
                }
            });
        });
    }
}

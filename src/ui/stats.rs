use eframe::egui;

use crate::app::SchedulerApp;
use crate::models::RunStatus;
use crate::theme::Theme;

impl SchedulerApp {
    pub fn render_stats(&self, ui: &mut egui::Ui) {
        let active  = self.jobs.iter().filter(|j| j.enabled).count();
        let success = self.jobs.iter().filter(|j| j.last_status == Some(RunStatus::Success)).count();
        let errors  = self.jobs.iter().filter(|j| j.last_status == Some(RunStatus::Error)).count();

        ui.columns(3, |cols| {
            for (i, (label, value, color)) in [
                ("ACTIVE",  active,  Theme::ACCENT),
                ("PASSING", success, Theme::SUCCESS),
                ("FAILING", errors,  Theme::ERROR),
            ].iter().enumerate() {
                egui::Frame::new()
                    .fill(Theme::PANEL)
                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(&mut cols[i], |ui| {
                        ui.label(egui::RichText::new(*label).size(10.0).color(Theme::TEXT_DIM));
                        ui.label(egui::RichText::new(format!("{}", value))
                            .size(26.0).strong().color(*color));
                    });
            }
        });
    }
}

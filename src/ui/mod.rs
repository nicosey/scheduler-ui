pub mod header;
pub mod stats;
pub mod job_list;
pub mod add_dialog;

use eframe::egui;

use crate::app::SchedulerApp;
use crate::theme::Theme;

impl eframe::App for SchedulerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut style = (*ctx.style()).clone();
        style.visuals.override_text_color = Some(Theme::TEXT);
        style.visuals.panel_fill = Theme::BG;
        style.visuals.window_fill = Theme::PANEL;
        style.visuals.widgets.noninteractive.bg_fill = Theme::PANEL;
        style.visuals.widgets.inactive.bg_fill = Theme::PANEL;
        style.visuals.widgets.inactive.weak_bg_fill = Theme::PANEL;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        ctx.set_style(style);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Theme::BG).inner_margin(egui::Margin::same(20)))
            .show(ctx, |ui| {
                self.render_header(ui);
                ui.add_space(12.0);
                self.render_stats(ui);
                ui.add_space(12.0);
                self.render_job_list(ui);
                ui.add_space(8.0);

                ui.vertical_centered(|ui| {
                    if let Some(msg) = &self.status_msg {
                        let color = if msg.starts_with("Error") { Theme::ERROR } else { Theme::SUCCESS };
                        ui.label(egui::RichText::new(msg).size(11.0).color(color));
                    } else {
                        let footer = if self.use_mock {
                            "Mock mode · changes not saved"
                        } else {
                            "Powered by cron + launchd · Click a job to expand"
                        };
                        ui.label(egui::RichText::new(footer).size(10.0).color(Theme::TEXT_DIM));
                    }
                });
            });

        if self.show_add_dialog {
            self.render_add_dialog(ctx);
        }
    }
}

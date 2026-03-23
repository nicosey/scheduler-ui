use eframe::egui;

pub struct Theme;

impl Theme {
    pub const BG: egui::Color32         = egui::Color32::from_rgb(15,  16,  23);
    pub const PANEL: egui::Color32      = egui::Color32::from_rgb(26,  27,  35);
    pub const BORDER: egui::Color32     = egui::Color32::from_rgb(42,  43,  53);
    pub const TEXT: egui::Color32       = egui::Color32::from_rgb(226, 232, 240);
    pub const TEXT_DIM: egui::Color32   = egui::Color32::from_rgb(100, 116, 139);
    pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(148, 163, 184);
    pub const ACCENT: egui::Color32     = egui::Color32::from_rgb(99,  102, 241);
    pub const SUCCESS: egui::Color32    = egui::Color32::from_rgb(52,  211, 153);
    pub const ERROR: egui::Color32      = egui::Color32::from_rgb(248, 113, 113);
    pub const SELECTED: egui::Color32   = egui::Color32::from_rgb(30,  31,  42);
    pub const LAUNCHD: egui::Color32    = egui::Color32::from_rgb(168, 85,  247);
}

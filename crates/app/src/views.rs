use crate::app::App;
use crate::theme;
use eframe::egui::Ui;

pub fn fmt_hm(minutes: i64) -> String {
    if minutes == 0 {
        return "0m".into();
    }
    let h = minutes / 60;
    let m = minutes % 60;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

impl App {
    pub fn view_timer(&mut self, ui: &mut Ui) {
        ui.label(theme::t("timer", 17.0));
    }

    pub fn view_tracker(&mut self, ui: &mut Ui) {
        ui.label(theme::t("tracker", 17.0));
    }

    pub fn view_summary(&mut self, ui: &mut Ui) {
        ui.label(theme::t("summary", 17.0));
    }

    pub fn view_log(&mut self, ui: &mut Ui) {
        ui.label(theme::t("log", 17.0));
    }
}

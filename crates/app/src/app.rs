use std::collections::HashSet;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
    Ui, Vec2,
};
use lockin_core::{Category, Db, PomoView, Task};

use crate::theme;
use crate::views;

pub static QUIT_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tasks,
    Tracker,
    Timer,
    Summary,
    Log,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Tasks => "tasks",
            View::Tracker => "tracker",
            View::Timer => "timer",
            View::Summary => "summary",
            View::Log => "log",
        }
    }
}

#[derive(Clone)]
pub struct Dialog {
    pub title: String,
    pub label: String,
    pub value: String,
    pub kind: DialogKind,
    pub error: Option<String>,
}

#[derive(Clone)]
pub enum DialogKind {
    AddGroup,
    RenameGroup(i64),
    RenameCategory(i64),
    AddSubject(i64),
    SnoozeDays(i64),
    ConfirmDeleteGroup(i64, String, usize),
    ConfirmDeleteSubject(i64, String),
    ConfirmDeleteCategory(i64, String),
}

pub struct App {
    pub db: Db,
    pub view: View,
    pub tasks: Vec<Task>,
    pub stats: (i64, i64),
    pub tasks_expanded: bool,
    pub done_collapsed: bool,
    pub add_open: bool,
    pub add_text: String,
    pub edit_id: Option<i64>,
    pub edit_text: String,
    pub edit_details: String,
    pub edit_saved_until: Option<Instant>,

    pub note: String,
    pub note_dirty: bool,
    pub note_edited_at: Option<Instant>,
    pub note_saved_until: Option<Instant>,

    pub pomo: PomoView,
    pub pomo_fetched: Instant,
    pub last_pomo_fetch: Instant,

    pub categories: Vec<Category>,
    pub subjects: Vec<(i64, String, String)>,
    pub settings_open: bool,
    pub cfg_focus: String,
    pub cfg_break: String,
    pub cfg_auto: bool,
    pub cfg_msg: Option<(String, bool, Instant)>,
    pub category_new: String,

    pub summary: Option<lockin_core::Summary>,
    pub months: Vec<String>,
    pub periods: Vec<lockin_core::PeriodRow>,
    pub periods_loaded: bool,
    pub periods_days: i64,
    pub import_open: bool,
    pub periods_open: bool,
    pub import_text: String,
    pub import_msg: String,

    pub tracker: Option<lockin_core::TrackerPayload>,
    pub group_expanded: HashSet<i64>,
    pub hold_collapsed: bool,
    pub done_collapsed_trk: bool,
    pub drop_collapsed: bool,
    pub tracker_editor: Option<i64>,
    pub tracker_menu: Option<(i64, Pos2)>,
    pub task_menu: Option<(i64, Pos2)>,
    pub drag_task: Option<i64>,
    pub drag_target: Option<i64>,
    pub add_subject_group: Option<i64>,
    pub add_subject_text: String,
    pub tracker_focus_edit: Option<String>,
    pub trk_name: String,
    pub trk_url: String,
    pub trk_notes: String,

    pub dialog: Option<Dialog>,
    pub hidden: bool,
    pub quitting: bool,
    pub ceramic: TextureHandle,
    pub arrow: TextureHandle,
    pub last_title: String,
    pub toast: Option<(String, Instant)>,
    pub export_month: String,
    pub zoom: f32,
    pub screenshot_path: Option<std::path::PathBuf>,
    pub frames: u64,
}

impl App {
    pub fn new(ctx: &egui::Context, db_path: std::path::PathBuf) -> Self {
        let db = Db::open(&db_path).expect("open lockin database");

        let ceramic = load_texture(ctx, "ceramic", include_bytes!("../../../assets/img/ceramic.gif"));
        let arrow = load_texture(ctx, "arrow", include_bytes!("../../../assets/img/arrow.gif"));

        let tasks = db.tasks().unwrap_or_default();
        let stats = db.todo_stats().unwrap_or((0, 0));
        let note = db.scratchpad();
        let pomo = db.pomo_view().unwrap_or_default();
        let categories = db.categories().unwrap_or_default();
        let subjects = load_subjects(&db);

        let month = chrono::Local::now().format("%Y-%m").to_string();
        let months = db.months().unwrap_or_default();
        let zoom: f32 = db
            .get_setting("ui_zoom")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.3)
            .clamp(0.9, 2.0);
        ctx.set_zoom_factor(zoom);
        let group_expanded: HashSet<i64> = db
            .get_setting("tracker_expanded_groups")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let hold_collapsed = db.get_setting_or("tracker_hold_collapsed", "1") == "1";
        let done_collapsed_trk = db.get_setting_or("tracker_done_collapsed", "1") == "1";
        let drop_collapsed = db.get_setting_or("tracker_drop_collapsed", "1") == "1";
        let initial_view = match std::env::var("LOCKIN_VIEW").unwrap_or_default().as_str() {
            "tracker" => View::Tracker,
            "timer" => View::Timer,
            "summary" => View::Summary,
            "log" => View::Log,
            _ => View::Tasks,
        };
        let mut app = Self {
            db,
            view: initial_view,
            tasks,
            stats,
            tasks_expanded: false,
            done_collapsed: true,
            add_open: false,
            add_text: String::new(),
            edit_id: None,
            edit_text: String::new(),
            edit_details: String::new(),
            edit_saved_until: None,
            note,
            note_dirty: false,
            note_edited_at: None,
            note_saved_until: None,
            pomo,
            pomo_fetched: Instant::now(),
            last_pomo_fetch: Instant::now(),
            categories,
            subjects,
            settings_open: false,
            cfg_focus: String::new(),
            cfg_break: String::new(),
            cfg_auto: false,
            cfg_msg: None,
            category_new: String::new(),
            summary: None,
            months,
            periods: Vec::new(),
            periods_loaded: false,
            periods_days: 60,
            import_open: false,
            periods_open: false,
            import_text: String::new(),
            import_msg: String::new(),
            tracker: None,
            group_expanded,
            hold_collapsed,
            done_collapsed_trk,
            drop_collapsed,
            tracker_editor: None,
            tracker_menu: None,
            task_menu: None,
            drag_task: None,
            drag_target: None,
            add_subject_group: None,
            add_subject_text: String::new(),
            tracker_focus_edit: None,
            trk_name: String::new(),
            trk_url: String::new(),
            trk_notes: String::new(),
            dialog: None,
            hidden: false,
            quitting: false,
            ceramic,
            arrow,
            last_title: String::new(),
            toast: None,
            export_month: month,
            zoom,
            screenshot_path: std::env::var("LOCKIN_SCREENSHOT")
                .ok()
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from),
            frames: 0,
        };
        if std::env::var("LOCKIN_DIALOG").ok().as_deref() == Some("group") {
            app.open_prompt(DialogKind::AddGroup, "add group", "group name (e.g. stream processing)", "");
        }
        if std::env::var("LOCKIN_LOG_OPEN").ok().as_deref() == Some("periods") {
            app.periods_open = true;
        }
        if std::env::var("LOCKIN_SETTINGS").ok().as_deref() == Some("1") {
            app.settings_open = true;
            app.cfg_focus = app.pomo.focus_min.to_string();
            app.cfg_break = app.pomo.break_min.to_string();
            app.cfg_auto = app.pomo.auto_next;
        }
        match app.view {
            View::Tracker => app.reload_tracker(),
            View::Summary => app.reload_summary(),
            View::Log => app.reload_periods(),
            _ => {}
        }
        app
    }

    // ---------------------------------------------------------------- data

    pub fn reload_tasks(&mut self) {
        self.tasks = self.db.tasks().unwrap_or_default();
        self.stats = self.db.todo_stats().unwrap_or((0, 0));
    }

    pub fn reload_timer(&mut self) {
        if let Ok(view) = self.db.pomo_view() {
            self.pomo = view;
            self.pomo_fetched = Instant::now();
            self.last_pomo_fetch = Instant::now();
        }
    }

    pub fn reload_categories(&mut self) {
        self.categories = self.db.categories().unwrap_or_default();
        self.subjects = load_subjects(&self.db);
    }

    pub fn save_tracker_collapse(&self) {
        if let Ok(json) = serde_json::to_string(&self.group_expanded) {
            let _ = self.db.set_setting("tracker_expanded_groups", &json);
        }
        let _ = self.db.set_setting(
            "tracker_hold_collapsed",
            if self.hold_collapsed { "1" } else { "0" },
        );
        let _ = self.db.set_setting(
            "tracker_done_collapsed",
            if self.done_collapsed_trk { "1" } else { "0" },
        );
        let _ = self.db.set_setting(
            "tracker_drop_collapsed",
            if self.drop_collapsed { "1" } else { "0" },
        );
    }

    pub fn reload_tracker(&mut self) {
        let payload = self.db.tracker_payload().ok();
        self.subjects = payload
            .as_ref()
            .map(|p| {
                p.groups
                    .iter()
                    .flat_map(|g| {
                        g.subjects.iter().filter(|s| s.state != "dropped").map(move |s| {
                            (s.id, s.name.clone(), g.name.clone())
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.tracker = payload;
    }

    pub fn reload_summary(&mut self) {
        self.summary = self.db.summary().ok();
        self.months = self.db.months().unwrap_or_default();
    }

    pub fn reload_periods(&mut self) {
        self.periods = self.db.periods(self.periods_days).unwrap_or_default();
        self.periods_loaded = true;
    }

    pub fn remaining_ms(&self) -> i64 {
        if self.pomo.running {
            let drift = self.pomo_fetched.elapsed().as_millis() as i64;
            (self.pomo.remaining_ms - drift).max(0)
        } else {
            self.pomo.remaining_ms.max(0)
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting || QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                let _ = std::fs::remove_file(crate::socket_path());
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.hidden = true;
            }
        }

        if !self.hidden {
            self.hotkeys(ctx);
            self.resize_edges(ctx);
        }

        if self.last_pomo_fetch.elapsed() >= Duration::from_millis(700) {
            self.reload_timer();
        }
        self.persist_note();

        if self.pomo.running || self.note_dirty {
            ctx.request_repaint_after(Duration::from_millis(250));
        } else {
            ctx.request_repaint_after(Duration::from_millis(1000));
        }

        self.frames += 1;
        self.update_title(ctx);
        self.paint_background(ctx);

        egui::TopBottomPanel::top("chrome")
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                let full = ui.available_width();
                let bar_rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    Vec2::new(full, 36.0),
                );
                ui.painter().rect_filled(
                    bar_rect,
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 64),
                );
                let drag = ui.interact(
                    bar_rect,
                    ui.id().with("window-drag"),
                    Sense::click_and_drag(),
                );
                if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() {
                    let maxed = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
                }
                ui.allocate_ui_with_layout(
                    Vec2::new(full, 36.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_space(16.0);
                        ui.spacing_mut().item_spacing.x = 10.0;
                        ui.label(theme::b("lock tf in", 16.0).color(theme::GOLD));
                        ui.label(
                            theme::i("est. black mesa research facility.", 9.0)
                                .color(theme::MUTED),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            self.window_controls(ui);
                            ui.add_space(14.0);
                        });
                    },
                );
                let screen = ui.ctx().screen_rect();
                let line_y = ui.cursor().top();
                ui.painter().line_segment(
                    [
                        egui::Pos2::new(screen.left(), line_y),
                        egui::Pos2::new(screen.right(), line_y),
                    ],
                    Stroke::new(1.0_f32, theme::BEVEL_HI),
                );
                ui.add_space(1.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(full, 34.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_space(16.0);
                        ui.spacing_mut().item_spacing.x = 18.0;
                        for (view, label) in [
                            (View::Tasks, "tasks"),
                            (View::Tracker, "tracker"),
                            (View::Timer, "timer"),
                            (View::Summary, "summary"),
                            (View::Log, "log"),
                        ] {
                            if theme::nav_link(ui, label, self.view == view).clicked() {
                                self.set_view(view);
                            }
                        }
                    },
                );
                ui.add_space(8.0);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                let full = ui.available_width();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let pad = 16.0;
                        let content_width = (full - pad * 2.0).max(240.0);
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(content_width);
                                ui.add_space(14.0);
                                match self.view {
                                    View::Tasks => self.view_tasks(ui),
                                    View::Tracker => self.view_tracker(ui),
                                    View::Timer => self.view_timer(ui),
                                    View::Summary => self.view_summary(ui),
                                    View::Log => self.view_log(ui),
                                }
                                ui.add_space(40.0);
                            });
                        });
                    });
            });

        // custom 1998 window bevel, no OS decorations
        let border = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("window-border"),
        ));
        let screen = ctx.screen_rect();
        border.rect_stroke(
            screen,
            0.0,
            Stroke::new(1.0_f32, egui::Color32::BLACK),
            egui::StrokeKind::Inside,
        );
        border.rect_stroke(
            screen.shrink(1.0),
            0.0,
            Stroke::new(1.0_f32, egui::Color32::from_rgb(0x3a, 0x24, 0x00)),
            egui::StrokeKind::Inside,
        );
        border.rect_stroke(
            screen.shrink(2.0),
            0.0,
            Stroke::new(1.0_f32, egui::Color32::BLACK),
            egui::StrokeKind::Inside,
        );

        self.task_menu_window(ctx);
        self.tracker_menu_window(ctx);
        if self.settings_open {
            self.settings_popup(ctx);
        }
        self.dialog_window(ctx);
        self.maybe_screenshot(ctx);

        if let Some((_, at)) = &self.toast {
            if at.elapsed() > Duration::from_secs(3) {
                self.toast = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
        }
    }

    fn maybe_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot_path.clone() else { return };
        if (2..=6).contains(&self.frames) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let [w, h] = image.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for p in &image.pixels {
                rgba.extend_from_slice(&p.to_array());
            }
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                let _ = buf.save(&path);
            }
            eprintln!("lockin: screenshot saved to {}", path.display());
            self.screenshot_path = None;
            self.quitting = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }

    fn hotkeys(&mut self, ctx: &egui::Context) {
        let typing = ctx.wants_keyboard_input();
        if ctx.input(|i| i.modifiers.ctrl) {
            if ctx.input(|i| i.key_pressed(egui::Key::Num1)) {
                self.set_view(View::Tasks);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num2)) {
                self.set_view(View::Tracker);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num3)) {
                self.set_view(View::Timer);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num4)) {
                self.set_view(View::Summary);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num5)) {
                self.set_view(View::Log);
            } else if ctx.input(|i| i.key_pressed(egui::Key::W)) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                self.hidden = true;
            } else if ctx.input(|i| i.key_pressed(egui::Key::Q)) {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
                self.set_zoom(ctx, self.zoom + 0.1);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
                self.set_zoom(ctx, self.zoom - 0.1);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num0)) {
                self.set_zoom(ctx, 1.3);
            }
        }
        if !typing && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            let action = if self.pomo.running {
                lockin_core::PomoAction::Pause
            } else {
                lockin_core::PomoAction::Start
            };
            let _ = self.db.pomo_action(action);
            self.reload_timer();
            self.reload_tasks();
        }
    }

    fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        self.zoom = (zoom * 100.0).round() / 100.0;
        self.zoom = self.zoom.clamp(0.9, 2.0);
        ctx.set_zoom_factor(self.zoom);
        let _ = self
            .db
            .set_setting("ui_zoom", &format!("{:.2}", self.zoom));
        ctx.request_repaint();
    }

    fn set_view(&mut self, view: View) {
        self.view = view;
        self.task_menu = None;
        self.tracker_menu = None;
        match view {
            View::Tracker => self.reload_tracker(),
            View::Summary => self.reload_summary(),
            View::Log => self.reload_periods(),
            _ => {}
        }
    }

    fn persist_note(&mut self) {
        if !self.note_dirty {
            return;
        }
        let due = self
            .note_edited_at
            .map(|t| t.elapsed() >= Duration::from_millis(600))
            .unwrap_or(true);
        if due {
            let _ = self.db.set_scratchpad(&self.note);
            self.note_dirty = false;
            self.note_saved_until = Some(Instant::now());
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let remaining = self.remaining_ms();
        let s = (remaining / 1000).max(0);
        let clock = format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60);
        let hm = views::fmt_hm(self.pomo.today_focused_min);
        let title = format!("{clock} · {hm} · grind");
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn paint_background(&self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let screen = ctx.screen_rect();
        let size = Vec2::splat(480.0);
        let mut y = screen.top();
        while y < screen.bottom() {
            let mut x = screen.left();
            while x < screen.right() {
                painter.image(
                    self.ceramic.id(),
                    Rect::from_min_size(Pos2::new(x, y), size),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                x += 480.0;
            }
            y += 480.0;
        }
    }

    // ---------------------------------------------------------------- chrome

    fn window_controls(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.x = 1.0;
        let close = wc_button(ui, "close");
        if close.clicked() {
            self.quitting = true;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let max = wc_button(ui, "max");
        if max.clicked() {
            let maxed = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maxed));
        }
        let min = wc_button(ui, "min");
        if min.clicked() {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn resize_edges(&mut self, ctx: &egui::Context) {
        use egui::ResizeDirection as D;
        let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        let r = ctx.screen_rect();
        let m = 6.0;
        let left = pos.x <= r.left() + m;
        let right = pos.x >= r.right() - m;
        let top = pos.y <= r.top() + m;
        let bottom = pos.y >= r.bottom() - m;
        let (dir, cursor) = match (left, right, top, bottom) {
            (true, _, true, _) => (D::NorthWest, egui::CursorIcon::ResizeNwSe),
            (_, true, true, _) => (D::NorthEast, egui::CursorIcon::ResizeNeSw),
            (true, _, _, true) => (D::SouthWest, egui::CursorIcon::ResizeNeSw),
            (_, true, _, true) => (D::SouthEast, egui::CursorIcon::ResizeNwSe),
            (true, _, _, _) => (D::West, egui::CursorIcon::ResizeHorizontal),
            (_, true, _, _) => (D::East, egui::CursorIcon::ResizeHorizontal),
            (_, _, true, _) => (D::North, egui::CursorIcon::ResizeVertical),
            (_, _, _, true) => (D::South, egui::CursorIcon::ResizeVertical),
            _ => return,
        };
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }

    fn navigation(&mut self, _ui: &mut Ui) {}

    pub fn section_title(ui: &mut Ui, left: &str, right: &str) {
        ui.horizontal(|ui| {
            ui.label(theme::b(left, 17.0).color(theme::GOLD));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                if !right.is_empty() {
                    ui.label(theme::i(right, 11.0).color(theme::MUTED));
                }
            });
        });
        ui.add_space(2.0);
    }

    // ---------------------------------------------------------------- tasks view

    fn view_tasks(&mut self, ui: &mut Ui) {
        let (done, total) = self.stats;
        let right = if total > 0 {
            format!("{done}/{total} done")
        } else {
            String::new()
        };
        Self::section_title(ui, "tasks", &right);
        ui.add_space(0.0);

        if self.tasks.is_empty() {
            theme::empty_note(ui, "no tasks yet", 16.0);
        } else {
            let open: Vec<Task> = self.tasks.iter().filter(|t| !t.is_done()).cloned().collect();
            let done_tasks: Vec<Task> = self.tasks.iter().filter(|t| t.is_done()).cloned().collect();
            let cap = if self.tasks_expanded { open.len() } else { open.len().min(4) };
            ui.spacing_mut().item_spacing.y = 0.0;
            for task in open.iter().take(cap) {
                self.task_row(ui, task);
            }
            if open.len() > 4 {
                ui.add_space(2.0);
                let label = if self.tasks_expanded {
                    "show fewer tasks"
                } else {
                    "show more (N hidden)"
                };
                let label = label.replace(
                    "N",
                    &(open.len() - 4).to_string(),
                );
                let caret = if self.tasks_expanded { theme::Caret::Down } else { theme::Caret::Right };
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    if theme::caret_link(ui, &label, caret).clicked() {
                        self.tasks_expanded = !self.tasks_expanded;
                    }
                });
            }
            if !done_tasks.is_empty() {
                ui.add_space(6.0);
                let label = format!("done ({})", done_tasks.len());
                let caret = if self.done_collapsed { theme::Caret::Right } else { theme::Caret::Down };
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    if theme::caret_link(ui, &label, caret).clicked() {
                        self.done_collapsed = !self.done_collapsed;
                    }
                });
                if !self.done_collapsed {
                    for task in &done_tasks {
                        self.task_row(ui, task);
                    }
                }
            }
        }

        ui.add_space(6.0);
        if self.add_open {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let width = ui.available_width();
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.add_text)
                        .hint_text("new task... (Enter)")
                        .desired_width(width - 80.0)
                        .font(theme::regular_font(13.0)),
                );
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.commit_add();
                }
                if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.add_open = false;
                    self.add_text.clear();
                }
                edit.request_focus();
                if theme::bevel(ui, "add", true).clicked() {
                    self.commit_add();
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let (rect, response) = ui.allocate_exact_size(Vec2::new(14.0, 18.0), Sense::click());
                let color = if response.hovered() { theme::RED_HOT } else { theme::RED };
                theme::paint_plus(ui.painter(), rect.center(), 11.0, color);
                if response.clicked() {
                    self.add_open = true;
                    self.add_text.clear();
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
        }

        ui.add_space(34.0);
        ui.horizontal(|ui| {
            ui.label(theme::b("day notes", 17.0).color(theme::GOLD));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                ui.label(
                    theme::i("lock the fuck in lil bitch", 11.0).color(theme::MUTED),
                );
            });
        });
        ui.add_space(13.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            let width = ui.available_width();
            let edit = ui.add(
                egui::TextEdit::multiline(&mut self.note)
                    .hint_text("one goal. lock in.")
                    .desired_width(width)
                    .desired_rows(4)
                    .frame(false)
                    .font(theme::italic_font(13.0)),
            );
            if edit.changed() {
                self.note_dirty = true;
                self.note_edited_at = Some(Instant::now());
            }
        });
        if self
            .note_saved_until
            .map(|t| t.elapsed() < Duration::from_millis(1100))
            .unwrap_or(false)
        {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(theme::tc("saved ✓", 12.0, theme::MUTED));
            });
        }
    }

    fn commit_add(&mut self) {
        let text = self.add_text.trim().to_string();
        if text.is_empty() {
            self.add_open = false;
            self.add_text.clear();
            return;
        }
        if self.db.add_task(&text, "").is_ok() {
            self.reload_tasks();
            self.add_text.clear();
            self.add_open = false;
        }
    }

    fn task_row(&mut self, ui: &mut Ui, task: &Task) {
        let width = ui.available_width();
        let indent = 16.0;
        let height = 20.0;
        let (row_rect, row_response) = ui.allocate_exact_size(
            Vec2::new(width, height),
            Sense::click_and_drag(),
        );
        let content = Rect::from_min_max(
            Pos2::new(row_rect.left() + indent, row_rect.top()),
            row_rect.right_bottom(),
        );
        let hovered = row_response.hovered();
        if hovered {
            ui.painter()
                .rect_filled(row_rect, 0.0, Color32::from_rgba_unmultiplied(215, 160, 90, 8));
        }
        if row_response.secondary_clicked() {
            self.task_menu = Some((task.id, ui.ctx().pointer_latest_pos().unwrap_or(row_rect.center())));
        }
        if row_response.drag_started() {
            self.drag_task = Some(task.id);
        }
        if self.drag_task.is_some() && hovered {
            self.drag_target = Some(task.id);
        }

        let glyph_rect = Rect::from_min_size(
            Pos2::new(content.left(), content.top()),
            Vec2::new(12.0, height),
        );
        let glyph_resp = ui.interact(
            glyph_rect,
            ui.id().with(("glyph", task.id)),
            Sense::click(),
        );
        if task.state == "doing" {
            let image_rect = Rect::from_min_size(
                Pos2::new(glyph_rect.left(), glyph_rect.center().y - 6.0),
                Vec2::new(9.0, 12.0),
            );
            ui.painter().image(
                self.arrow.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            let color = if task.is_done() { theme::DIM } else { theme::GOLD };
            let galley = ui.painter().layout_no_wrap(
                "·".into(),
                FontId::new(15.0, theme::regular_family()),
                color,
            );
            ui.painter().galley(
                Pos2::new(
                    glyph_rect.center().x - galley.size().x / 2.0,
                    glyph_rect.center().y - galley.size().y / 2.0,
                ),
                galley,
                color,
            );
        }
        if glyph_resp.clicked() {
            let next = match task.state.as_str() {
                "todo" => "doing",
                "doing" => "done",
                _ => "todo",
            };
            let _ = self.db.patch_task(task.id, None, None, Some(next));
            self.reload_tasks();
            self.reload_timer();
        }
        if glyph_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        let text_x = glyph_rect.right() + 9.0;
        let (text_color, family) = if task.is_done() {
            (theme::DIM, theme::regular_family())
        } else if task.state == "doing" {
            (theme::BRIGHT, theme::bold_family())
        } else {
            (theme::GOLD, theme::regular_family())
        };
        let galley = ui.painter().layout_no_wrap(
            task.text.clone(),
            FontId::new(13.0, family),
            text_color,
        );
        let text_rect = Rect::from_min_size(
            Pos2::new(text_x, row_rect.top() + (height - galley.size().y) / 2.0),
            galley.size(),
        );
        ui.painter()
            .galley(text_rect.min, galley.clone(), text_color);
        if task.is_done() {
            theme::strike(ui.painter(), text_rect, theme::DIM);
        }
        if !task.details.is_empty() {
            let pencil_rect = Rect::from_center_size(
                Pos2::new(text_rect.right() + 8.0, text_rect.center().y),
                Vec2::new(9.0, 9.0),
            );
            theme::paint_pencil(ui.painter(), pencil_rect, theme::MUTED);
        }
        let text_resp = ui.interact(text_rect, ui.id().with(("text", task.id)), Sense::click());
        if text_resp.clicked() {
            self.open_editor(task);
        }
        if text_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // delete x
        let x_rect = Rect::from_min_size(
            Pos2::new(content.right() - 18.0, row_rect.top()),
            Vec2::new(16.0, height),
        );
        let x_resp = ui.interact(x_rect, ui.id().with(("del", task.id)), Sense::click());
        let x_color = if x_resp.hovered() {
            theme::RED_HOT
        } else if hovered {
            theme::MUTED
        } else {
            theme::DIM
        };
        theme::paint_x(ui.painter(), x_rect.center(), 7.0, x_color);
        if x_resp.clicked() {
            let _ = self.db.delete_task(task.id);
            self.reload_tasks();
            if self.edit_id == Some(task.id) {
                self.edit_id = None;
            }
        }
        if x_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if self.edit_id == Some(task.id) {
            self.task_editor(ui, task);
        }

        if ui.input(|i| i.pointer.any_released()) {
            if let (Some(from), Some(to)) = (self.drag_task, self.drag_target) {
                if from != to {
                    let mut order: Vec<i64> = self
                        .tasks
                        .iter()
                        .filter(|t| !t.is_done())
                        .map(|t| t.id)
                        .collect();
                    if let (Some(f), Some(t)) = (
                        order.iter().position(|id| *id == from),
                        order.iter().position(|id| *id == to),
                    ) {
                        let id = order.remove(f);
                        order.insert(t, id);
                        let _ = self.db.reorder_tasks(&order);
                        self.reload_tasks();
                    }
                }
            }
            self.drag_task = None;
            self.drag_target = None;
        }
    }

    fn open_editor(&mut self, task: &Task) {
        if self.edit_id == Some(task.id) {
            self.edit_id = None;
            return;
        }
        self.edit_id = Some(task.id);
        self.edit_text = task.text.clone();
        self.edit_details = task.details.clone();
    }

    fn task_editor(&mut self, ui: &mut Ui, task: &Task) {
        ui.horizontal(|ui| {
            ui.add_space(37.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 20.0).max(160.0));
                ui.label(theme::i("text", 12.0).color(theme::MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.edit_text)
                        .desired_width(f32::INFINITY)
                        .font(theme::regular_font(13.0)),
                );
                ui.label(theme::i("details", 12.0).color(theme::MUTED));
                ui.add(
                    egui::TextEdit::multiline(&mut self.edit_details)
                        .hint_text("notes, commands, links...")
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .font(theme::regular_font(13.0)),
                );
                ui.horizontal(|ui| {
                    if theme::link(ui, "save", 13.0, theme::RED).clicked() {
                        if self.db.patch_task(
                            task.id,
                            Some(&self.edit_text),
                            Some(&self.edit_details),
                            None,
                        ).is_ok()
                        {
                            self.reload_tasks();
                            self.edit_saved_until = Some(Instant::now());
                        }
                    }
                    if self
                        .edit_saved_until
                        .map(|t| t.elapsed() < Duration::from_millis(1200))
                        .unwrap_or(false)
                    {
                        ui.label(theme::tc("saved ✓", 12.0, theme::MUTED));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if theme::link(ui, "close", 12.0, theme::MUTED).clicked() {
                            self.edit_id = None;
                        }
                    });
                });
                ui.add_space(6.0);
            });
        });
    }

    fn task_menu_window(&mut self, ctx: &egui::Context) {
        let Some((id, pos)) = self.task_menu else { return };
        let mut close = false;
        let mut new_state = None;
        let mut delete = false;
        egui::Area::new(egui::Id::new("task-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                menu_frame(ui, self.ceramic.id(), |ui| {
                    ui.label(theme::tc("set state", 11.0, theme::MUTED));
                    if menu_item(ui, "working on it") {
                        new_state = Some("doing");
                        close = true;
                    }
                    if menu_item(ui, "not working on it") {
                        new_state = Some("todo");
                        close = true;
                    }
                    if menu_item(ui, "done") {
                        new_state = Some("done");
                        close = true;
                    }
                    ui.separator();
                    if menu_item(ui, "delete") {
                        delete = true;
                        close = true;
                    }
                });
            });
        if let Some(state) = new_state {
            let _ = self.db.patch_task(id, None, None, Some(state));
            self.reload_tasks();
            self.reload_timer();
        }
        if delete {
            let _ = self.db.delete_task(id);
            self.reload_tasks();
            if self.edit_id == Some(id) {
                self.edit_id = None;
            }
        }
        if close || ctx.input(|i| i.pointer.any_click()) && !ctx.is_pointer_over_area() {
            self.task_menu = None;
        }
    }

    fn tracker_menu_window(&mut self, ctx: &egui::Context) {
        let Some((id, pos)) = self.tracker_menu else { return };
        let subject = self.tracker.as_ref().and_then(|t| {
            t.groups
                .iter()
                .flat_map(|g| g.subjects.iter())
                .find(|s| s.id == id)
                .cloned()
        });
        let Some(subject) = subject else {
            self.tracker_menu = None;
            return;
        };
        let mut close = false;
        let mut set_state: Option<&'static str> = None;
        let mut clear_snooze = false;
        let mut delete = false;
        egui::Area::new(egui::Id::new("tracker-menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                menu_frame(ui, self.ceramic.id(), |ui| {
                    if subject.focused_min > 0 {
                        ui.label(theme::tc(
                            format!("{} focused", views::fmt_hm(subject.focused_min)),
                            11.0,
                            theme::MUTED,
                        ));
                    }
                    if subject.state == "paused" {
                        if menu_item(ui, "resume") {
                            set_state = Some("doing");
                            close = true;
                        }
                    } else if !matches!(subject.state.as_str(), "done" | "dropped") {
                        if menu_item(ui, "on hold") {
                            set_state = Some("paused");
                            close = true;
                        }
                    }
                    if subject.state == "dropped" {
                        if menu_item(ui, "restore") {
                            set_state = Some("todo");
                            close = true;
                        }
                    } else if !matches!(subject.state.as_str(), "done" | "dropped") {
                        if menu_item(ui, "drop this") {
                            set_state = Some("dropped");
                            close = true;
                        }
                    }
                    if subject.snoozed {
                        if menu_item(ui, "clear snooze") {
                            clear_snooze = true;
                            close = true;
                        }
                    }
                    if menu_item(ui, "delete") {
                        delete = true;
                        close = true;
                    }
                });
            });
        if let Some(state) = set_state {
            let _ = self.db.subject_state(id, state);
            self.reload_tracker();
        }
        if clear_snooze {
            let _ = self.db.snooze_subject(id, None, None, "", true);
            self.reload_tracker();
        }
        if delete {
            self.open_prompt(
                DialogKind::ConfirmDeleteSubject(id, subject.name.clone()),
                "delete subject",
                &format!(
                    "delete \"{}\"? its progress log and focus-time link go too.",
                    subject.name
                ),
                "",
            );
        }
        if close || (ctx.input(|i| i.pointer.any_click()) && !ctx.is_pointer_over_area()) {
            self.tracker_menu = None;
        }
    }

    pub fn open_prompt(&mut self, kind: DialogKind, title: &str, label: &str, value: &str) {
        self.dialog = Some(Dialog {
            title: title.to_string(),
            label: label.to_string(),
            value: value.to_string(),
            kind,
            error: None,
        });
    }

    fn dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let confirm_only = matches!(
            dialog.kind,
            DialogKind::ConfirmDeleteGroup(..)
                | DialogKind::ConfirmDeleteSubject(..)
                | DialogKind::ConfirmDeleteCategory(..)
        );
        let mut action: Option<bool> = None;
        egui::Area::new(egui::Id::new("lockin-dialog"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fade_in(false)
            .show(ctx, |ui| {
                theme::textured_panel(
                    ui,
                    self.ceramic.id(),
                    Some(if confirm_only { 360.0 } else { 340.0 }),
                    egui::Margin::symmetric(16, 12),
                    |ui| {
                    ui.horizontal(|ui| {
                        ui.label(theme::b(dialog.title.clone(), 13.0).color(theme::BRIGHT));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if popup_close(ui) {
                                action = Some(false);
                            }
                        });
                    });
                    ui.add_space(8.0);
                    ui.label(theme::t(dialog.label.clone(), 12.5).color(theme::GOLD));
                    if !confirm_only {
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut dialog.value)
                                .desired_width(f32::INFINITY)
                                .font(theme::regular_font(13.0)),
                        );
                        edit.request_focus();
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            action = Some(true);
                        }
                    }
                    if let Some(error) = &dialog.error {
                        ui.label(theme::tc(error, 12.0, theme::RED_HOT));
                    }
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let yes = if confirm_only { "delete" } else { "ok" };
                        if theme::bevel(ui, yes, true).clicked() {
                            action = Some(true);
                        }
                        if theme::bevel(ui, "cancel", false).clicked() {
                            action = Some(false);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            action = Some(false);
                        }
                    });
                });
            });

        match action {
            None => {
                self.dialog = Some(dialog);
            }
            Some(false) => {}
            Some(true) => match self.apply_dialog(&dialog) {
                Ok(()) => {}
                Err(e) => {
                    dialog.error = Some(e.to_string());
                    self.dialog = Some(dialog);
                }
            },
        }
    }

    fn apply_dialog(&mut self, dialog: &Dialog) -> anyhow::Result<()> {
        let text = dialog.value.trim().to_string();
        match &dialog.kind {
            DialogKind::AddGroup => {
                anyhow::ensure!(!text.is_empty(), "name required");
                self.db.add_group(&text)?;
                self.reload_tracker();
            }
            DialogKind::RenameGroup(id) => {
                anyhow::ensure!(!text.is_empty(), "name required");
                self.db.rename_group(*id, &text)?;
                self.reload_tracker();
            }
            DialogKind::RenameCategory(id) => {
                anyhow::ensure!(!text.is_empty(), "name required");
                self.db.rename_category(*id, &text)?;
                self.reload_categories();
            }
            DialogKind::AddSubject(group) => {
                anyhow::ensure!(!text.is_empty(), "name required");
                self.db.add_subject(*group, &text, "other", "", "", None)?;
                self.reload_tracker();
            }
            DialogKind::SnoozeDays(id) => {
                let days: i64 = text.parse()?;
                self.db.snooze_subject(*id, Some(days), None, "", false)?;
                self.reload_tracker();
            }
            DialogKind::ConfirmDeleteGroup(id, _, _) => {
                self.db.delete_group(*id)?;
                self.reload_tracker();
            }
            DialogKind::ConfirmDeleteSubject(id, _) => {
                self.db.delete_subject(*id)?;
                self.reload_tracker();
                if self.tracker_editor == Some(*id) {
                    self.tracker_editor = None;
                }
            }
            DialogKind::ConfirmDeleteCategory(id, _) => {
                self.db.delete_category(*id)?;
                self.reload_categories();
                self.reload_timer();
            }
        }
        Ok(())
    }
}

fn load_subjects(db: &Db) -> Vec<(i64, String, String)> {
    let mut out = Vec::new();
    if let Ok(payload) = db.tracker_payload() {
        for g in payload.groups {
            for s in g.subjects {
                if s.state != "dropped" {
                    out.push((s.id, s.name, g.name.clone()));
                }
            }
        }
    }
    out
}

fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let mut options = TextureOptions::NEAREST;
            if name == "ceramic" {
                options.wrap_mode = egui::TextureWrapMode::Repeat;
            }
            ctx.load_texture(name, color, options)
        }
        Err(_) => ctx.load_texture(
            name,
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0x28, 0x18, 0x00, 255]),
            TextureOptions::NEAREST,
        ),
    }
}

pub fn wc_button(ui: &mut Ui, kind: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(24.0, 20.0), Sense::click());
    let color = if response.hovered() {
        theme::RED
    } else {
        theme::MUTED
    };
    match kind {
        "min" => {
            ui.painter().line_segment(
                [
                    Pos2::new(rect.center().x - 4.5, rect.center().y),
                    Pos2::new(rect.center().x + 4.5, rect.center().y),
                ],
                Stroke::new(1.0_f32, color),
            );
        }
        "max" => {
            ui.painter().rect_stroke(
                Rect::from_center_size(rect.center(), Vec2::splat(8.0)),
                0.0,
                Stroke::new(1.0_f32, color),
                egui::StrokeKind::Middle,
            );
        }
        _ => {
            theme::paint_x(ui.painter(), rect.center(), 8.0, color);
        }
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn popup_close(ui: &mut Ui) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::click());
    let color = if response.hovered() {
        theme::RED_HOT
    } else {
        theme::MUTED
    };
    theme::paint_x(ui.painter(), rect.center(), 8.0, color);
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

pub fn menu_frame<R>(ui: &mut Ui, tex: egui::TextureId, add: impl FnOnce(&mut Ui) -> R) -> R {
    theme::textured_panel(ui, tex, None, egui::Margin::symmetric(6, 5), |ui| {
        ui.set_min_width(132.0);
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
        add(ui)
    })
}

pub fn menu_item(ui: &mut Ui, text: &str) -> bool {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, theme::regular_family()),
        theme::RED,
    );
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), galley.size().y + 6.0),
        Sense::click(),
    );
    let color = if response.hovered() { theme::RED_HOT } else { theme::RED };
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(215, 160, 90, 10));
    }
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        FontId::new(13.0, theme::regular_family()),
        color,
    );
    ui.painter().galley(
        Pos2::new(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        App::update(self, ctx, frame);
    }
}


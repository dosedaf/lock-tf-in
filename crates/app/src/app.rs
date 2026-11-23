use std::collections::HashSet;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions,
    Ui, Vec2,
};
use lockin_core::{Category, Db, PeriodSnapshot, PomoView, Task};

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
    Settings,
}

#[derive(Clone)]
pub enum UndoAction {
    Task(Task),
    Period(PeriodSnapshot),
    PeriodDelete(i64),
    SubjectState {
        id: i64,
        state: String,
        started_at: Option<String>,
        finished_at: Option<String>,
        last_touched: Option<String>,
    },
    SubjectProgress {
        id: i64,
        unit_done: f64,
        state: String,
        log_id: Option<i64>,
    },
    Snooze {
        id: i64,
        until: Option<String>,
        reason: String,
    },
    TaskOrder(Vec<i64>),
    SubjectOrder(Vec<i64>),
    GroupOrder(Vec<i64>),
}

#[derive(Clone)]
pub enum PaletteAction {
    Switch(View),
    AddTask,
    ToggleTimer,
    ToggleTop,
    EditTask(i64),
    EditSubject(i64),
    LogTime,
}

#[derive(Clone)]
pub struct PaletteItem {
    pub label: String,
    pub hint: String,
    pub action: PaletteAction,
}

pub struct Toast {
    pub text: String,
    pub at: Instant,
    pub undo: bool,
}

#[derive(Clone)]
pub struct Dialog {
    pub title: String,
    pub label: String,
    pub value: String,
    pub kind: DialogKind,
    pub error: Option<String>,
    pub needs_focus: bool,
}

#[derive(Clone)]
pub enum DialogKind {
    AddGroup,
    RenameGroup(i64),
    RenameCategory(i64),
    SnoozeDays(i64),
    ConfirmDeleteGroup(i64),
    ConfirmDeleteSubject(i64),
    ConfirmDeleteCategory(i64),
}

pub struct App {
    pub db: Db,
    pub view: View,
    pub tasks: Vec<Task>,
    pub stats: (i64, i64),
    pub tasks_expanded: bool,
    pub done_collapsed: bool,
    pub add_open: bool,
    pub add_focus: bool,
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
    pub cat_merge_from: Option<i64>,
    pub cat_merge_to: Option<i64>,
    pub cat_unalloc_to: Option<i64>,

    pub summary: Option<lockin_core::Summary>,
    pub summary_range: Option<i64>,
    pub months: Vec<String>,
    pub periods: Vec<lockin_core::PeriodRow>,
    pub periods_loaded: bool,
    pub periods_days: i64,
    pub import_open: bool,
    pub periods_open: bool,
    pub import_text: String,
    pub import_msg: String,
    pub log_date: String,
    pub log_start: String,
    pub log_end: String,
    pub log_minutes: String,
    pub log_category: Option<i64>,
    pub log_subject: Option<i64>,
    pub log_editing: Option<i64>,
    pub log_msg: Option<(String, bool, Instant)>,

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
    pub add_subject_focus: bool,
    pub add_subject_text: String,
    pub trk_name: String,
    pub trk_url: String,
    pub trk_notes: String,
    pub trk_set: String,

    pub dialog: Option<Dialog>,
    pub hidden: bool,
    pub quitting: bool,
    pub ceramic: TextureHandle,
    pub arrow: TextureHandle,
    pub last_title: String,
    pub toast: Option<Toast>,
    pub last_undo: Option<UndoAction>,
    pub export_month: String,
    pub zoom: f32,
    pub settings_w: String,
    pub settings_h: String,
    pub settings_zoom: String,
    pub settings_msg: Option<(String, bool, Instant)>,
    pub settings_needs_sync: bool,
    pub last_window: (u32, u32),
    pub pending_window: (u32, u32),
    pub window_save_at: Option<Instant>,
    pub window_started_at: Instant,
    pub last_tasks_refresh: Instant,
    pub last_view_refresh: Instant,
    pub always_on_top: bool,
    pub window_level_applied: bool,
    pub settings_opened_frame: u64,
    pub palette_open: bool,
    pub palette_query: String,
    pub palette_index: usize,
    pub palette_focus: bool,
    pub drag_group: Option<i64>,
    pub drag_group_target: Option<i64>,
    pub drag_subject: Option<(i64, i64)>,
    pub drag_subject_target: Option<i64>,
    pub screenshot_path: Option<std::path::PathBuf>,
    pub frames: u64,
}

impl App {
    pub fn new(ctx: &egui::Context, db_path: std::path::PathBuf) -> Self {
        let db = Db::open(&db_path).expect("open lockin database");

        let ceramic = load_texture(
            ctx,
            "ceramic",
            include_bytes!("../../../assets/img/ceramic.gif"),
        );
        let arrow = load_texture(
            ctx,
            "arrow",
            include_bytes!("../../../assets/img/arrow.gif"),
        );

        let tasks = db.tasks().unwrap_or_default();
        let stats = db.todo_stats().unwrap_or((0, 0));
        let note = db.scratchpad();
        let pomo = db.pomo_view().unwrap_or_default();
        let categories = db.categories().unwrap_or_default();
        let subjects = load_subjects(&db);

        let month = chrono::Local::now().format("%Y-%m").to_string();
        let months = db.months().unwrap_or_default();
        let saved_w = db
            .get_setting("ui_window_w")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(960)
            .clamp(640, 4000);
        let saved_h = db
            .get_setting("ui_window_h")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(780)
            .clamp(620, 3000);
        let always_on_top = db.get_setting_or("ui_always_on_top", "0") == "1";
        let zoom: f32 = db
            .get_setting("ui_zoom")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.55)
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
            "settings" => View::Settings,
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
            add_focus: false,
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
            cat_merge_from: None,
            cat_merge_to: None,
            cat_unalloc_to: None,
            summary: None,
            summary_range: Some(30),
            months,
            periods: Vec::new(),
            periods_loaded: false,
            periods_days: 60,
            import_open: false,
            periods_open: false,
            import_text: String::new(),
            import_msg: String::new(),
            log_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            log_start: String::new(),
            log_end: String::new(),
            log_minutes: String::new(),
            log_category: None,
            log_subject: None,
            log_editing: None,
            log_msg: None,
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
            add_subject_focus: false,
            add_subject_text: String::new(),
            trk_name: String::new(),
            trk_url: String::new(),
            trk_notes: String::new(),
            trk_set: String::new(),
            dialog: None,
            hidden: false,
            quitting: false,
            ceramic,
            arrow,
            last_title: String::new(),
            toast: None,
            last_undo: None,
            export_month: month,
            zoom,
            settings_w: saved_w.to_string(),
            settings_h: saved_h.to_string(),
            settings_zoom: format!("{:.0}", zoom * 100.0),
            settings_msg: None,
            settings_needs_sync: true,
            last_window: (saved_w, saved_h),
            pending_window: (saved_w, saved_h),
            window_save_at: None,
            window_started_at: Instant::now(),
            last_tasks_refresh: Instant::now(),
            last_view_refresh: Instant::now(),
            always_on_top,
            window_level_applied: false,
            settings_opened_frame: 0,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focus: false,
            drag_group: None,
            drag_group_target: None,
            drag_subject: None,
            drag_subject_target: None,
            screenshot_path: std::env::var("LOCKIN_SCREENSHOT")
                .ok()
                .filter(|s| !s.is_empty())
                .map(std::path::PathBuf::from),
            frames: 0,
        };
        if std::env::var("LOCKIN_DIALOG").ok().as_deref() == Some("group") {
            app.open_prompt(
                DialogKind::AddGroup,
                "add group",
                "group name (e.g. stream processing)",
                "",
            );
        }
        if std::env::var("LOCKIN_PALETTE").ok().as_deref() == Some("1") {
            app.palette_open = true;
            app.palette_focus = true;
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
                        g.subjects
                            .iter()
                            .filter(|s| s.state != "dropped")
                            .map(move |s| (s.id, s.name.clone(), g.name.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.tracker = payload;
    }

    pub fn reload_summary(&mut self) {
        self.summary = self.db.summary_range(self.summary_range).ok();
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
        self.toast = Some(Toast {
            text: text.into(),
            at: Instant::now(),
            undo: false,
        });
    }

    pub fn set_undo(&mut self, action: UndoAction, text: impl Into<String>) {
        self.last_undo = Some(action);
        self.toast = Some(Toast {
            text: text.into(),
            at: Instant::now(),
            undo: true,
        });
    }

    pub fn subject_state_undo(&self, subject: &lockin_core::TrackerSubject) -> UndoAction {
        UndoAction::SubjectState {
            id: subject.id,
            state: subject.state.clone(),
            started_at: subject.started_at.clone(),
            finished_at: subject.finished_at.clone(),
            last_touched: subject.last_touched.clone(),
        }
    }

    pub fn progress_undo(&self, subject: &lockin_core::TrackerSubject) -> Option<UndoAction> {
        let log_id = self.db.last_progress_log_id(subject.id).ok().flatten();
        Some(UndoAction::SubjectProgress {
            id: subject.id,
            unit_done: subject.unit_done,
            state: subject.state.clone(),
            log_id,
        })
    }

    fn perform_undo(&mut self) {
        let Some(action) = self.last_undo.take() else {
            return;
        };
        match action {
            UndoAction::Task(task) => {
                let _ = self.db.restore_task(&task);
                self.reload_tasks();
            }
            UndoAction::Period(p) => {
                let _ = self.db.restore_period(&p);
                self.reload_periods();
                self.reload_timer();
            }
            UndoAction::PeriodDelete(id) => {
                let _ = self.db.delete_period(id);
                self.reload_periods();
                self.reload_timer();
            }
            UndoAction::SubjectState {
                id,
                state,
                started_at,
                finished_at,
                last_touched,
            } => {
                let _ = self.db.restore_subject_state(
                    id,
                    &state,
                    started_at.as_deref(),
                    finished_at.as_deref(),
                    last_touched.as_deref(),
                );
                self.reload_tracker();
            }
            UndoAction::SubjectProgress {
                id,
                unit_done,
                state,
                log_id,
            } => {
                let _ = self
                    .db
                    .restore_subject_progress(id, unit_done, &state, log_id);
                self.reload_tracker();
            }
            UndoAction::Snooze { id, until, reason } => {
                let _ = self.db.restore_snooze(id, until.as_deref(), &reason);
                self.reload_tracker();
            }
            UndoAction::TaskOrder(order) => {
                let _ = self.db.reorder_tasks(&order);
                self.reload_tasks();
            }
            UndoAction::SubjectOrder(order) => {
                let _ = self.db.reorder_subjects(&order);
                self.reload_tracker();
            }
            UndoAction::GroupOrder(order) => {
                let _ = self.db.reorder_groups(&order);
                self.reload_tracker();
            }
        }
        self.toast("undone");
    }

    fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_focus = self.palette_open;
    }

    fn palette_items(&self) -> Vec<PaletteItem> {
        let mut items = vec![
            PaletteItem {
                label: "tasks".into(),
                hint: "view".into(),
                action: PaletteAction::Switch(View::Tasks),
            },
            PaletteItem {
                label: "tracker".into(),
                hint: "view".into(),
                action: PaletteAction::Switch(View::Tracker),
            },
            PaletteItem {
                label: "timer".into(),
                hint: "view".into(),
                action: PaletteAction::Switch(View::Timer),
            },
            PaletteItem {
                label: "summary".into(),
                hint: "view".into(),
                action: PaletteAction::Switch(View::Summary),
            },
            PaletteItem {
                label: "log".into(),
                hint: "view".into(),
                action: PaletteAction::Switch(View::Log),
            },
            PaletteItem {
                label: "settings".into(),
                hint: "view".into(),
                action: PaletteAction::Switch(View::Settings),
            },
            PaletteItem {
                label: "add task".into(),
                hint: "tasks".into(),
                action: PaletteAction::AddTask,
            },
            PaletteItem {
                label: "toggle timer".into(),
                hint: "start / pause".into(),
                action: PaletteAction::ToggleTimer,
            },
            PaletteItem {
                label: "log time".into(),
                hint: "manual entry".into(),
                action: PaletteAction::LogTime,
            },
            PaletteItem {
                label: format!(
                    "always on top: {}",
                    if self.always_on_top { "on" } else { "off" }
                ),
                hint: "window".into(),
                action: PaletteAction::ToggleTop,
            },
        ];
        for t in self.tasks.iter().take(80) {
            items.push(PaletteItem {
                label: format!("task · {}", t.text),
                hint: "edit".into(),
                action: PaletteAction::EditTask(t.id),
            });
        }
        for (id, name, group) in self.subjects.iter().take(80) {
            items.push(PaletteItem {
                label: format!("subject · {name}"),
                hint: group.clone(),
                action: PaletteAction::EditSubject(*id),
            });
        }
        items
    }

    fn run_palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::Switch(view) => self.set_view(view),
            PaletteAction::AddTask => {
                self.set_view(View::Tasks);
                self.add_open = true;
                self.add_focus = true;
                self.add_text.clear();
            }
            PaletteAction::ToggleTimer => {
                let action = if self.pomo.running {
                    lockin_core::PomoAction::Pause
                } else {
                    lockin_core::PomoAction::Start
                };
                let _ = self.db.pomo_action(action);
                self.reload_timer();
                self.reload_tasks();
            }
            PaletteAction::ToggleTop => {
                self.always_on_top = !self.always_on_top;
                let _ = self.db.set_setting(
                    "ui_always_on_top",
                    if self.always_on_top { "1" } else { "0" },
                );
                self.set_top_level(ctx);
                self.toast(if self.always_on_top {
                    "always on top enabled"
                } else {
                    "always on top disabled"
                });
            }
            PaletteAction::EditTask(id) => {
                self.set_view(View::Tasks);
                if let Some(task) = self.tasks.iter().find(|t| t.id == id).cloned() {
                    self.open_editor(&task);
                }
            }
            PaletteAction::EditSubject(id) => {
                self.set_view(View::Tracker);
                if let Some(subject) = self.tracker.as_ref().and_then(|t| {
                    t.groups
                        .iter()
                        .flat_map(|g| g.subjects.iter())
                        .find(|s| s.id == id)
                        .cloned()
                }) {
                    self.tracker_editor = Some(id);
                    self.trk_name = subject.name;
                    self.trk_url = subject.url;
                    self.trk_notes = subject.notes;
                    self.trk_set.clear();
                }
            }
            PaletteAction::LogTime => {
                self.set_view(View::Log);
                self.reload_periods();
            }
        }
        self.palette_open = false;
    }

    fn palette_window(&mut self, ctx: &egui::Context) {
        if !self.palette_open {
            return;
        }
        let mut execute: Option<PaletteAction> = None;
        let mut close = false;
        let items = self.palette_items();
        let query = self.palette_query.to_lowercase();
        let filtered: Vec<&PaletteItem> = items
            .iter()
            .filter(|i| {
                query.is_empty()
                    || i.label.to_lowercase().contains(&query)
                    || i.hint.to_lowercase().contains(&query)
            })
            .take(60)
            .collect();
        if filtered.is_empty() {
            self.palette_index = 0;
        } else if self.palette_index >= filtered.len() {
            self.palette_index = filtered.len() - 1;
        }
        if !filtered.is_empty()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown))
        {
            self.palette_index = (self.palette_index + 1) % filtered.len();
        }
        if !filtered.is_empty()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp))
        {
            self.palette_index = (self.palette_index + filtered.len() - 1) % filtered.len();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            if let Some(item) = filtered.get(self.palette_index) {
                execute = Some(item.action.clone());
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }

        let selected = self.palette_index;
        let focus = self.palette_focus;
        egui::Area::new(egui::Id::new("palette"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 70.0])
            .fade_in(false)
            .show(ctx, |ui| {
                theme::textured_panel(
                    ui,
                    self.ceramic.id(),
                    Some(520.0),
                    egui::Margin::symmetric(12, 10),
                    |ui| {
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut self.palette_query)
                                .hint_text("type a command or search…")
                                .desired_width(f32::INFINITY)
                                .font(theme::regular_font(13.0)),
                        );
                        if focus {
                            edit.request_focus();
                            self.palette_focus = false;
                        }
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical()
                            .max_height(280.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for (idx, item) in filtered.iter().enumerate() {
                                    let is_selected = idx == selected;
                                    let text = if item.hint.is_empty() {
                                        item.label.clone()
                                    } else {
                                        format!("{}   ·   {}", item.label, item.hint)
                                    };
                                    let resp = ui.selectable_label(
                                        is_selected,
                                        theme::t(text, 12.5).color(if is_selected {
                                            theme::BRIGHT
                                        } else {
                                            theme::GOLD
                                        }),
                                    );
                                    if resp.clicked() {
                                        execute = Some(item.action.clone());
                                    }
                                }
                            });
                        ui.add_space(4.0);
                        ui.label(
                            theme::i("↑ ↓ select · enter run · esc close", 10.5).color(theme::DIM),
                        );
                    },
                );
            });
        if let Some(action) = execute {
            self.run_palette_action(ctx, action);
        }
        if close {
            self.palette_open = false;
        }
    }

    fn set_top_level(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.always_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
    }

    fn toast_window(&mut self, ctx: &egui::Context) {
        let Some(toast) = &self.toast else {
            return;
        };
        if toast.at.elapsed() > Duration::from_secs(5) {
            self.toast = None;
            self.last_undo = None;
            return;
        }
        let text = toast.text.clone();
        let can_undo = toast.undo;
        let mut clicked = false;
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-18.0, -18.0])
            .fade_in(false)
            .show(ctx, |ui| {
                theme::textured_panel(
                    ui,
                    self.ceramic.id(),
                    None,
                    egui::Margin::symmetric(12, 8),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.label(theme::t(text, 12.0).color(theme::BRIGHT));
                            if can_undo && theme::link(ui, "undo", 12.0, theme::RED).clicked() {
                                clicked = true;
                            }
                        });
                    },
                );
            });
        if clicked {
            self.perform_undo();
        }
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.quitting || QUIT_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                self.save_window_now(ctx);
                self.flush_note();
                let _ = std::fs::remove_file(crate::socket_path());
                let _ = std::fs::remove_file(crate::pid_path());
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
        if !self.window_level_applied {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            }));
            self.window_level_applied = true;
        }

        if self.last_pomo_fetch.elapsed() >= Duration::from_millis(700) {
            self.reload_timer();
        }
        self.persist_note();
        self.track_window(ctx);
        if self.last_tasks_refresh.elapsed() >= Duration::from_millis(1200) {
            self.reload_tasks();
            self.last_tasks_refresh = Instant::now();
        }
        if self.last_view_refresh.elapsed() >= Duration::from_millis(2000) {
            match self.view {
                View::Tracker => self.reload_tracker(),
                View::Summary => self.reload_summary(),
                View::Log if self.periods_loaded => self.reload_periods(),
                _ => {}
            }
            self.last_view_refresh = Instant::now();
        }

        if self.pomo.running || self.note_dirty {
            ctx.request_repaint_after(Duration::from_millis(250));
        } else {
            ctx.request_repaint_after(Duration::from_millis(1000));
        }

        self.frames += 1;
        self.update_title(ctx);
        self.paint_background(ctx);

        egui::TopBottomPanel::top("chrome")
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                let full = ui.available_width();
                let bar_rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(full, 36.0));
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
                        ui.add_space(24.0);
                        ui.spacing_mut().item_spacing.x = 10.0;
                        ui.label(theme::b("lock tf in", 16.0).color(theme::GOLD));
                        ui.label(
                            theme::i("est. black mesa research facility.", 9.0).color(theme::MUTED),
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
                        ui.add_space(24.0);
                        ui.spacing_mut().item_spacing.x = 14.0;
                        for (view, label) in [
                            (View::Tasks, "tasks"),
                            (View::Tracker, "tracker"),
                            (View::Timer, "timer"),
                            (View::Summary, "summary"),
                            (View::Log, "log"),
                            (View::Settings, "settings"),
                        ] {
                            if theme::nav_link(ui, label, self.view == view).clicked() {
                                self.set_view(view);
                            }
                        }
                    },
                );
                ui.add_space(3.0);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                let full = ui.available_width();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let pad = 24.0;
                        let content_width = (full - pad * 2.0).max(240.0);
                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_width(content_width);
                                ui.add_space(3.0);
                                match self.view {
                                    View::Tasks => self.view_tasks(ui),
                                    View::Tracker => self.view_tracker(ui),
                                    View::Timer => self.view_timer(ui),
                                    View::Summary => self.view_summary(ui),
                                    View::Log => self.view_log(ui),
                                    View::Settings => self.view_settings(ui),
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
        self.palette_window(ctx);
        self.toast_window(ctx);
        self.maybe_screenshot(ctx);
    }

    fn maybe_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot_path.clone() else {
            return;
        };
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
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::K)) {
            self.toggle_palette();
            return;
        }
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
            } else if ctx.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::R)) {
                self.reset_window(ctx);
            } else if ctx
                .input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals))
            {
                self.set_zoom(ctx, self.zoom + 0.1);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
                self.set_zoom(ctx, self.zoom - 0.1);
            } else if ctx.input(|i| i.key_pressed(egui::Key::Num0)) {
                self.set_zoom(ctx, 1.55);
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

    pub fn apply_window_size(&mut self, ctx: &egui::Context, w: f32, h: f32) {
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            self.settings_msg = Some((
                "width and height must be positive".into(),
                false,
                Instant::now(),
            ));
            return;
        }
        let (mut w, mut h) = (w, h);
        let mut clamped = false;
        if let Some(mon) = ctx.input(|i| i.viewport().monitor_size) {
            let max_w = (mon.x * 0.92).max(1.0);
            let max_h = (mon.y * 0.88).max(1.0);
            if w > max_w {
                w = max_w;
                clamped = true;
            }
            if h > max_h {
                h = max_h;
                clamped = true;
            }
        }
        let wi = w.round().max(1.0) as u32;
        let hi = h.round().max(1.0) as u32;
        self.settings_w = wi.to_string();
        self.settings_h = hi.to_string();
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(w, h)));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.last_window = (wi, hi);
        self.pending_window = (wi, hi);
        self.window_save_at = None;
        let _ = self.db.set_setting("ui_window_w", &wi.to_string());
        let _ = self.db.set_setting("ui_window_h", &hi.to_string());
        self.settings_msg = Some((
            if clamped {
                format!("clamped to screen · saved {wi} × {hi}")
            } else {
                format!("window size saved · {wi} × {hi}")
            },
            true,
            Instant::now(),
        ));
    }

    fn reset_window(&mut self, ctx: &egui::Context) {
        self.apply_window_size(ctx, 960.0, 780.0);
    }

    pub fn clamp_menu_pos(
        &self,
        ctx: &egui::Context,
        pos: egui::Pos2,
        width: f32,
        height: f32,
    ) -> egui::Pos2 {
        let screen = ctx.screen_rect();
        let x = pos.x.clamp(
            screen.left() + 4.0,
            (screen.right() - width - 4.0).max(screen.left() + 4.0),
        );
        let y = pos.y.clamp(
            screen.top() + 4.0,
            (screen.bottom() - height - 4.0).max(screen.top() + 4.0),
        );
        egui::Pos2::new(x, y)
    }

    pub fn native_window_size(&self, ctx: &egui::Context) -> (u32, u32) {
        let size = ctx.screen_rect().size() * self.zoom;
        (
            size.x.round().max(1.0) as u32,
            size.y.round().max(1.0) as u32,
        )
    }

    fn track_window(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if self.window_started_at.elapsed() < Duration::from_millis(1500) {
            return;
        }
        let size = self.native_window_size(ctx);
        if size.0 == 0 || size.1 == 0 || size.0 > 5000 || size.1 > 4000 {
            return;
        }
        if size != self.pending_window {
            self.pending_window = size;
            self.window_save_at = Some(now + Duration::from_millis(500));
        }
        if let Some(at) = self.window_save_at {
            if now >= at && self.pending_window != self.last_window {
                self.last_window = self.pending_window;
                self.window_save_at = None;
                let _ = self
                    .db
                    .set_setting("ui_window_w", &self.last_window.0.to_string());
                let _ = self
                    .db
                    .set_setting("ui_window_h", &self.last_window.1.to_string());
            }
        }
    }

    fn save_window_now(&mut self, ctx: &egui::Context) {
        let size = self.native_window_size(ctx);
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        self.last_window = size;
        self.pending_window = size;
        let _ = self.db.set_setting("ui_window_w", &size.0.to_string());
        let _ = self.db.set_setting("ui_window_h", &size.1.to_string());
    }

    pub fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        self.zoom = (zoom * 100.0).round() / 100.0;
        self.zoom = self.zoom.clamp(0.9, 2.0);
        ctx.set_zoom_factor(self.zoom);
        let _ = self.db.set_setting("ui_zoom", &format!("{:.2}", self.zoom));
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
            View::Settings => {
                self.settings_needs_sync = true;
                self.reload_categories();
            }
            _ => {}
        }
    }

    fn flush_note(&mut self) {
        if self.note_dirty {
            let _ = self.db.set_scratchpad(&self.note);
            self.note_dirty = false;
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
            let open: Vec<Task> = self
                .tasks
                .iter()
                .filter(|t| !t.is_done())
                .cloned()
                .collect();
            let done_tasks: Vec<Task> =
                self.tasks.iter().filter(|t| t.is_done()).cloned().collect();
            let cap = if self.tasks_expanded {
                open.len()
            } else {
                open.len().min(4)
            };
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
                let label = label.replace("N", &(open.len() - 4).to_string());
                let caret = if self.tasks_expanded {
                    theme::Caret::Down
                } else {
                    theme::Caret::Right
                };
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
                let caret = if self.done_collapsed {
                    theme::Caret::Right
                } else {
                    theme::Caret::Down
                };
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
                if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.commit_add();
                }
                if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.add_open = false;
                    self.add_focus = false;
                    self.add_text.clear();
                }
                if self.add_focus {
                    edit.request_focus();
                    self.add_focus = false;
                }
                if theme::bevel(ui, "add", true).clicked() {
                    self.commit_add();
                }
            });
        } else {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(14.0, 18.0), Sense::click());
                let color = if response.hovered() {
                    theme::RED_HOT
                } else {
                    theme::RED
                };
                theme::paint_plus(ui.painter(), rect.center(), 11.0, color);
                if response.clicked() {
                    self.add_open = true;
                    self.add_focus = true;
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
                ui.label(theme::i("lock the fuck in lil bitch", 11.0).color(theme::MUTED));
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
            self.add_focus = false;
            self.add_text.clear();
            return;
        }
        if self.db.add_task(&text, "").is_ok() {
            self.reload_tasks();
            self.add_text.clear();
            self.add_open = false;
            self.add_focus = false;
        }
    }

    fn task_row(&mut self, ui: &mut Ui, task: &Task) {
        let width = ui.available_width();
        let indent = 16.0;
        let height = 20.0;
        let (row_rect, row_response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        let content = Rect::from_min_max(
            Pos2::new(row_rect.left() + indent, row_rect.top()),
            row_rect.right_bottom(),
        );
        let hovered = row_response.hovered();
        if hovered {
            ui.painter().rect_filled(
                row_rect,
                0.0,
                Color32::from_rgba_unmultiplied(215, 160, 90, 8),
            );
        }
        if row_response.secondary_clicked() {
            let pos = ui.ctx().pointer_latest_pos().unwrap_or(row_rect.center());
            self.task_menu = Some((task.id, self.clamp_menu_pos(ui.ctx(), pos, 180.0, 160.0)));
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
        let glyph_resp = ui.interact(glyph_rect, ui.id().with(("glyph", task.id)), Sense::click());
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
            let color = if task.is_done() {
                theme::DIM
            } else {
                theme::GOLD
            };
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
            let prev = task.clone();
            if self.db.patch_task(task.id, None, None, Some(next)).is_ok() {
                self.set_undo(UndoAction::Task(prev), "task state changed");
                self.reload_tasks();
                self.reload_timer();
            }
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
        let galley =
            ui.painter()
                .layout_no_wrap(task.text.clone(), FontId::new(13.0, family), text_color);
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
        let text_resp = ui.interact(
            text_rect,
            ui.id().with(("text", task.id)),
            Sense::click_and_drag(),
        );
        if text_resp.drag_started() {
            self.drag_task = Some(task.id);
        }
        if self.drag_task.is_some() && text_resp.hovered() {
            self.drag_target = Some(task.id);
        }
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
            let prev = self
                .db
                .task_by_id(task.id)
                .ok()
                .flatten()
                .unwrap_or_else(|| task.clone());
            if self.db.delete_task(task.id).is_ok() {
                self.set_undo(UndoAction::Task(prev), "task deleted");
                self.reload_tasks();
                if self.edit_id == Some(task.id) {
                    self.edit_id = None;
                }
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
                        let before = order.clone();
                        let id = order.remove(f);
                        order.insert(t, id);
                        if self.db.reorder_tasks(&order).is_ok() {
                            self.set_undo(UndoAction::TaskOrder(before), "tasks reordered");
                            self.reload_tasks();
                        }
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
                        let prev = task.clone();
                        if self
                            .db
                            .patch_task(
                                task.id,
                                Some(&self.edit_text),
                                Some(&self.edit_details),
                                None,
                            )
                            .is_ok()
                        {
                            self.set_undo(UndoAction::Task(prev), "task updated");
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
        let Some((id, pos)) = self.task_menu else {
            return;
        };
        let mut close = false;
        let mut new_state = None;
        let mut delete = false;
        let area = egui::Area::new(egui::Id::new("task-menu"))
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
            let prev = self.db.task_by_id(id).ok().flatten();
            if self.db.patch_task(id, None, None, Some(state)).is_ok() {
                if let Some(prev) = prev {
                    self.set_undo(UndoAction::Task(prev), "task state changed");
                }
                self.reload_tasks();
                self.reload_timer();
            }
        }
        if delete {
            let prev = self.db.task_by_id(id).ok().flatten();
            if self.db.delete_task(id).is_ok() {
                if let Some(prev) = prev {
                    self.set_undo(UndoAction::Task(prev), "task deleted");
                }
                self.reload_tasks();
                if self.edit_id == Some(id) {
                    self.edit_id = None;
                }
            }
        }
        let menu_rect = area.response.rect.expand(8.0);
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let clicked_outside = ctx.input(|i| i.pointer.any_click())
            && pointer.map(|p| !menu_rect.contains(p)).unwrap_or(false);
        if close || clicked_outside {
            self.task_menu = None;
        }
    }

    fn tracker_menu_window(&mut self, ctx: &egui::Context) {
        let Some((id, pos)) = self.tracker_menu else {
            return;
        };
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
        let area = egui::Area::new(egui::Id::new("tracker-menu"))
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
                    } else if !matches!(subject.state.as_str(), "done" | "dropped")
                        && menu_item(ui, "on hold")
                    {
                        set_state = Some("paused");
                        close = true;
                    }
                    if subject.state == "dropped" {
                        if menu_item(ui, "restore") {
                            set_state = Some("todo");
                            close = true;
                        }
                    } else if !matches!(subject.state.as_str(), "done" | "dropped")
                        && menu_item(ui, "drop this")
                    {
                        set_state = Some("dropped");
                        close = true;
                    }
                    if subject.snoozed && menu_item(ui, "clear snooze") {
                        clear_snooze = true;
                        close = true;
                    }
                    if menu_item(ui, "delete") {
                        delete = true;
                        close = true;
                    }
                });
            });
        if let Some(state) = set_state {
            let undo = self.subject_state_undo(&subject);
            if self.db.subject_state(id, state).is_ok() {
                self.set_undo(undo, "tracker state changed");
                self.reload_tracker();
            }
        }
        if clear_snooze {
            let undo = UndoAction::Snooze {
                id,
                until: subject.snooze_until.clone(),
                reason: subject.snooze_reason.clone(),
            };
            if self.db.snooze_subject(id, None, None, "", true).is_ok() {
                self.set_undo(undo, "snooze cleared");
                self.reload_tracker();
            }
        }
        if delete {
            self.open_prompt(
                DialogKind::ConfirmDeleteSubject(id),
                "delete subject",
                &format!(
                    "delete \"{}\"? its progress log and focus-time link go too.",
                    subject.name
                ),
                "",
            );
        }
        let menu_rect = area.response.rect.expand(8.0);
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let clicked_outside = ctx.input(|i| i.pointer.any_click())
            && pointer.map(|p| !menu_rect.contains(p)).unwrap_or(false);
        if close || clicked_outside {
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
            needs_focus: true,
        });
    }

    fn dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
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
                            if dialog.needs_focus {
                                edit.request_focus();
                                dialog.needs_focus = false;
                            }
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
                    },
                );
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
            DialogKind::SnoozeDays(id) => {
                let days: i64 = text.parse()?;
                let prev = self.tracker.as_ref().and_then(|t| {
                    t.groups
                        .iter()
                        .flat_map(|g| g.subjects.iter())
                        .find(|s| s.id == *id)
                        .cloned()
                });
                self.db.snooze_subject(*id, Some(days), None, "", false)?;
                if let Some(prev) = prev {
                    self.set_undo(
                        UndoAction::Snooze {
                            id: *id,
                            until: prev.snooze_until,
                            reason: prev.snooze_reason,
                        },
                        "snoozed",
                    );
                }
                self.reload_tracker();
            }
            DialogKind::ConfirmDeleteGroup(id) => {
                self.db.delete_group(*id)?;
                self.reload_tracker();
            }
            DialogKind::ConfirmDeleteSubject(id) => {
                self.db.delete_subject(*id)?;
                self.reload_tracker();
                if self.tracker_editor == Some(*id) {
                    self.tracker_editor = None;
                }
            }
            DialogKind::ConfirmDeleteCategory(id) => {
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
    let color = if response.hovered() {
        theme::RED_HOT
    } else {
        theme::RED
    };
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

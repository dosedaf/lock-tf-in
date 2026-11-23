use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct Task {
    pub id: i64,
    pub day_id: i64,
    pub text: String,
    pub details: String,
    pub state: String,
    pub position: i64,
    pub done_at: Option<String>,
}

impl Task {
    pub fn is_done(&self) -> bool {
        self.state == "done"
    }
}

#[derive(Clone, Debug)]
pub struct TodoStats {
    pub done: i64,
    pub total: i64,
}

#[derive(Clone, Debug)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub total_min: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PomoState {
    pub phase: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub elapsed_ms: i64,
    pub duration_min: i64,
    pub category_id: Option<i64>,
    pub block_category_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub block_subject_id: Option<i64>,
    pub block_day_id: Option<i64>,
    pub phase_started_at: Option<String>,
    pub logged_ms: i64,
}

impl Default for PomoState {
    fn default() -> Self {
        Self {
            phase: "focus".into(),
            running: false,
            started_at: None,
            elapsed_ms: 0,
            duration_min: 50,
            category_id: None,
            block_category_id: None,
            subject_id: None,
            block_subject_id: None,
            block_day_id: None,
            phase_started_at: None,
            logged_ms: 0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PomoConfig {
    pub focus_min: i64,
    pub break_min: i64,
    pub auto_next: bool,
}

#[derive(Clone, Debug)]
pub struct NamedId {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct PomoView {
    pub phase: String,
    pub running: bool,
    pub ended: bool,
    pub elapsed_ms: i64,
    pub remaining_ms: i64,
    pub duration_min: i64,
    pub focus_min: i64,
    pub break_min: i64,
    pub auto_next: bool,
    pub category: Option<NamedId>,
    pub subject: Option<NamedId>,
    pub today_focused_min: i64,
    pub today_periods: i64,
}

#[derive(Clone, Debug, Default)]
pub struct TrackerSubject {
    pub id: i64,
    pub group_id: i64,
    pub name: String,
    pub kind: String,
    pub url: String,
    pub unit_label: String,
    pub unit_total: Option<f64>,
    pub unit_done: f64,
    pub state: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_touched: Option<String>,
    pub snooze_until: Option<String>,
    pub snooze_reason: String,
    pub is_next: bool,
    pub notes: String,
    pub days_idle: Option<i64>,
    pub stale: bool,
    pub snoozed: bool,
    pub focused_min: i64,
}

#[derive(Clone, Debug, Default)]
pub struct TrackerGroup {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub pct: Option<i64>,
    pub done: i64,
    pub count: i64,
    pub subjects: Vec<TrackerSubject>,
}

#[derive(Clone, Debug, Default)]
pub struct TrackerTotals {
    pub active: i64,
    pub stale: i64,
    pub units_left: f64,
    pub focused_min: i64,
}

#[derive(Clone, Debug, Default)]
pub struct TrackerPayload {
    pub stale_days: i64,
    pub focus: String,
    pub groups: Vec<TrackerGroup>,
    pub totals: TrackerTotals,
}

#[derive(Clone, Debug, Default)]
pub struct DailyCategory {
    pub date: String,
    pub label: String,
    pub total: i64,
    pub cats: Vec<(String, i64)>,
}

#[derive(Clone, Debug, Default)]
pub struct HeatmapDay {
    pub date: String,
    pub minutes: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Bucket {
    pub label: String,
    pub min: i64,
}

#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub total_min: i64,
    pub total_periods: i64,
    pub days_accessed: i64,
    pub streak_current: i64,
    pub streak_best: i64,
    pub avg_per_focus_day: i64,
    pub daily_categories: Vec<DailyCategory>,
    pub weeks: Vec<Bucket>,
    pub months: Vec<Bucket>,
    pub hours: Vec<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct PeriodSnapshot {
    pub id: i64,
    pub session_id: Option<i64>,
    pub day_id: Option<i64>,
    pub category_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub task_id: Option<i64>,
    pub task_label: String,
    pub minutes: i64,
    pub started_at: String,
    pub ended_at: String,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub struct PeriodRow {
    pub id: i64,
    pub started_at: String,
    pub ended_at: String,
    pub minutes: i64,
    pub category_id: Option<i64>,
    pub subject_id: Option<i64>,
    pub category_name: Option<String>,
    pub task_label: String,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub struct ImportPreview {
    pub project: String,
    pub minutes: i64,
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Default)]
pub struct ImportResult {
    pub parsed: usize,
    pub imported: usize,
    pub skipped: usize,
    pub preview: Vec<ImportPreview>,
}

#[derive(Clone, Debug)]
pub enum CardKind {
    Day,
    Week,
    Month { year: i32, month: u32 },
}

#[derive(Clone, Debug, Default)]
pub struct ProgressEntry {
    pub logged_at: String,
    pub delta: f64,
    pub unit_done_after: f64,
    pub note: String,
}

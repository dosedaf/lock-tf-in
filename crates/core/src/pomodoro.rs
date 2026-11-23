use anyhow::{bail, Result};
use chrono::{Duration, Local};
use rusqlite::{params, OptionalExtension};

use crate::db::{now_iso, parse_iso, today_str, Db};
use crate::model::{NamedId, PomoConfig, PomoState, PomoView};

pub enum PomoAction {
    Start,
    Pause,
    Reset,
    Skip,
    SetCategory(Option<i64>),
    SetSubject(Option<i64>),
    Config {
        focus_min: i64,
        break_min: i64,
        auto_next: bool,
    },
}

impl Db {
    pub fn pomo_config(&self) -> PomoConfig {
        let focus_min = self
            .get_setting("pomo_focus_min")
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);
        let break_min = self
            .get_setting("pomo_break_min")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);
        PomoConfig {
            focus_min,
            break_min,
            auto_next: self.get_setting_or("pomo_auto_next", "0") == "1",
        }
    }

    fn default_pomo_state(&self, cfg: &PomoConfig) -> PomoState {
        PomoState {
            duration_min: cfg.focus_min,
            ..Default::default()
        }
    }

    pub fn pomo_state(&self) -> PomoState {
        let cfg = self.pomo_config();
        let mut st = self.default_pomo_state(&cfg);
        if let Some(raw) = self.get_setting("pomo_state") {
            if let Ok(parsed) = serde_json::from_str::<PomoState>(&raw) {
                st = parsed;
            }
        }
        if !st.running && st.elapsed_ms == 0 {
            st.duration_min = if st.phase == "focus" {
                cfg.focus_min
            } else {
                cfg.break_min
            };
        }
        st
    }

    fn save_pomo_state(&self, st: &PomoState) -> Result<()> {
        self.set_setting("pomo_state", &serde_json::to_string(st)?)
    }

    fn pomo_elapsed_ms(&self, st: &PomoState) -> i64 {
        let mut elapsed = st.elapsed_ms.max(0);
        if st.running {
            if let Some(started) = st.started_at.as_deref().and_then(parse_iso) {
                let now = Local::now().naive_local();
                elapsed += (now - started).num_milliseconds();
            }
        }
        elapsed.max(0)
    }

    fn new_phase(&self, st: &mut PomoState, phase: &str, running: bool) -> Result<()> {
        let cfg = self.pomo_config();
        st.phase = phase.to_string();
        st.duration_min = if phase == "focus" {
            cfg.focus_min
        } else {
            cfg.break_min
        };
        st.elapsed_ms = 0;
        st.running = running;
        st.started_at = if running { Some(now_iso()) } else { None };
        st.phase_started_at = Some(now_iso());
        st.logged_ms = 0;
        Ok(())
    }

    fn log_period(
        &self,
        st: &PomoState,
        minutes: i64,
        started_at: &str,
        ended_at: &str,
    ) -> Result<()> {
        let cat = st.block_category_id.or(st.category_id);
        let day = st.block_day_id.unwrap_or_else(|| self.current_day_id());
        let sub = st.block_subject_id.or(st.subject_id);
        self.conn.execute(
            "INSERT OR IGNORE INTO pomo_periods \
             (session_id, day_id, category_id, subject_id, task_id, task_label, minutes, started_at, ended_at, source) \
             VALUES (?,?,?,?,?,?,?,?,?, 'timer')",
            params![
                Option::<i64>::None,
                day,
                cat,
                sub,
                Option::<i64>::None,
                "",
                minutes,
                started_at,
                ended_at
            ],
        )?;
        if let Some(sub) = sub {
            self.touch_subject(sub)?;
        }
        Ok(())
    }

    fn log_partial(&self, st: &mut PomoState) -> Result<()> {
        if st.phase != "focus" || st.phase_started_at.is_none() {
            return Ok(());
        }
        let elapsed = self.pomo_elapsed_ms(st);
        let logged = st.logged_ms;
        let unlogged = elapsed - logged;
        if unlogged < 60_000 {
            return Ok(());
        }
        let started = st
            .started_at
            .clone()
            .or_else(|| st.phase_started_at.clone())
            .unwrap_or_else(now_iso);
        self.log_period(
            st,
            ((unlogged as f64) / 60_000.0).round().max(1.0) as i64,
            &started,
            &now_iso(),
        )?;
        st.logged_ms = elapsed;
        Ok(())
    }

    pub fn pomo_tick(&self) -> Result<PomoView> {
        let mut st = self.pomo_state();
        if !st.running || st.started_at.is_none() {
            return self.pomo_view_from(&st);
        }
        let elapsed = self.pomo_elapsed_ms(&st);
        let duration = st.duration_min * 60_000;
        if elapsed < duration {
            return self.pomo_view_from(&st);
        }
        if st.phase == "focus" {
            let logged = st.logged_ms;
            let rem_ms = (duration - logged).max(60_000);
            let p_started = st.started_at.clone().unwrap_or_else(now_iso);
            let p_ended = parse_iso(&p_started)
                .map(|t| {
                    (t + Duration::milliseconds(rem_ms))
                        .format("%Y-%m-%dT%H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(now_iso);
            self.log_period(
                &st,
                ((rem_ms as f64) / 60_000.0).round().max(1.0) as i64,
                &p_started,
                &p_ended,
            )?;
        }
        let auto = self.pomo_config().auto_next;
        notify_phase(&st.phase, &self.path, !auto);
        let next = if st.phase == "focus" {
            "break"
        } else {
            "focus"
        };
        self.new_phase(&mut st, next, auto)?;
        self.save_pomo_state(&st)?;
        self.pomo_view_from(&st)
    }

    pub fn pomo_view(&self) -> Result<PomoView> {
        let st = self.pomo_state();
        self.pomo_view_from(&st)
    }

    fn pomo_view_from(&self, st: &PomoState) -> Result<PomoView> {
        let cfg = self.pomo_config();
        let elapsed = self.pomo_elapsed_ms(st);
        let duration = st.duration_min * 60_000;
        let remaining = (duration - elapsed).max(0);

        let category = if let Some(id) = st.category_id {
            self.conn
                .query_row(
                    "SELECT id, name FROM categories WHERE id=?",
                    params![id],
                    |r| {
                        Ok(NamedId {
                            id: r.get(0)?,
                            name: r.get(1)?,
                        })
                    },
                )
                .optional()?
        } else {
            None
        };

        let subject = if let Some(id) = st.subject_id {
            self.conn
                .query_row(
                    "SELECT s.id, s.name FROM tracker_subjects s WHERE s.id=?",
                    params![id],
                    |r| {
                        Ok(NamedId {
                            id: r.get(0)?,
                            name: r.get(1)?,
                        })
                    },
                )
                .optional()?
        } else {
            None
        };

        let (mut today_min, today_periods): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(minutes),0), COUNT(*) FROM pomo_periods \
             WHERE substr(started_at,1,10)=?",
            params![today_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if st.phase == "focus" {
            if let Some(ps) = st.phase_started_at.as_deref() {
                if ps.len() >= 10 && ps[..10] == today_str() {
                    let unlogged = self.pomo_elapsed_ms(st) - st.logged_ms;
                    if unlogged > 0 {
                        today_min += ((unlogged as f64) / 60_000.0).round() as i64;
                    }
                }
            }
        }

        Ok(PomoView {
            phase: st.phase.clone(),
            running: st.running,
            ended: !st.running && remaining <= 0,
            elapsed_ms: elapsed,
            remaining_ms: remaining,
            duration_min: st.duration_min,
            focus_min: cfg.focus_min,
            break_min: cfg.break_min,
            auto_next: cfg.auto_next,
            category,
            subject,
            today_focused_min: today_min,
            today_periods,
        })
    }

    pub fn pomo_action(&self, action: PomoAction) -> Result<PomoView> {
        let mut st = self.pomo_state();
        match action {
            PomoAction::Start => {
                let elapsed = self.pomo_elapsed_ms(&st);
                if elapsed >= st.duration_min * 60_000 || st.phase_started_at.is_none() {
                    self.log_partial(&mut st)?;
                    let phase = st.phase.clone();
                    self.new_phase(&mut st, &phase, true)?;
                    st.block_category_id = st.category_id;
                    st.block_subject_id = st.subject_id;
                    st.block_day_id = Some(self.current_day_id());
                } else {
                    st.running = true;
                    st.started_at = Some(now_iso());
                    if st.elapsed_ms == 0 {
                        st.block_category_id = st.category_id;
                        st.block_subject_id = st.subject_id;
                        st.block_day_id = Some(self.current_day_id());
                    }
                }
                self.save_pomo_state(&st)?;
            }
            PomoAction::Pause => {
                if st.running {
                    self.log_partial(&mut st)?;
                    st.elapsed_ms = self.pomo_elapsed_ms(&st);
                    st.running = false;
                    st.started_at = None;
                }
                self.save_pomo_state(&st)?;
            }
            PomoAction::Reset => {
                self.log_partial(&mut st)?;
                st.elapsed_ms = 0;
                st.running = false;
                st.started_at = None;
                st.phase_started_at = None;
                st.logged_ms = 0;
                self.save_pomo_state(&st)?;
            }
            PomoAction::Skip => {
                self.log_partial(&mut st)?;
                let next = if st.phase == "focus" {
                    "break"
                } else {
                    "focus"
                };
                self.new_phase(&mut st, next, false)?;
                self.save_pomo_state(&st)?;
            }
            PomoAction::SetCategory(cid) => {
                if let Some(cid) = cid {
                    let exists = self
                        .conn
                        .query_row("SELECT 1 FROM categories WHERE id=?", params![cid], |r| {
                            r.get::<_, i64>(0)
                        })
                        .optional()?;
                    if exists.is_none() {
                        bail!("category not found");
                    }
                }
                st.category_id = cid;
                self.save_pomo_state(&st)?;
            }
            PomoAction::SetSubject(sid) => {
                if let Some(sid) = sid {
                    let exists = self
                        .conn
                        .query_row(
                            "SELECT 1 FROM tracker_subjects WHERE id=?",
                            params![sid],
                            |r| r.get::<_, i64>(0),
                        )
                        .optional()?;
                    if exists.is_none() {
                        bail!("subject not found");
                    }
                }
                st.subject_id = sid;
                self.save_pomo_state(&st)?;
            }
            PomoAction::Config {
                focus_min,
                break_min,
                auto_next,
            } => {
                if !(1..=600).contains(&focus_min) {
                    bail!("focus_min must be 1..600");
                }
                if !(1..=600).contains(&break_min) {
                    bail!("break_min must be 1..600");
                }
                self.set_setting("pomo_focus_min", &focus_min.to_string())?;
                self.set_setting("pomo_break_min", &break_min.to_string())?;
                self.set_setting("pomo_auto_next", if auto_next { "1" } else { "0" })?;
                if !st.running && st.elapsed_ms == 0 {
                    st.duration_min = if st.phase == "focus" {
                        focus_min
                    } else {
                        break_min
                    };
                    self.save_pomo_state(&st)?;
                }
            }
        }
        self.pomo_view()
    }
}

pub fn notify_phase(phase: &str, db_path: &std::path::Path, offer_start: bool) {
    let msg = if phase == "focus" {
        "focus done"
    } else {
        "break over"
    };
    let _ = std::process::Command::new("canberra-gtk-play")
        .args(["-i", "dialog-information"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    let mut notification = notify_rust::Notification::new();
    notification
        .summary("grind")
        .body(msg)
        .appname("grind")
        .timeout(notify_rust::Timeout::Milliseconds(20000));
    if offer_start {
        notification.action("start", "start next");
    }
    match notification.show() {
        Ok(handle) => {
            if offer_start {
                let path = db_path.to_path_buf();
                std::thread::spawn(move || {
                    handle.wait_for_action(|action| {
                        if action == "start" {
                            if let Ok(db) = Db::open(&path) {
                                let _ = db.pomo_action(PomoAction::Start);
                            }
                        }
                    });
                });
            }
        }
        Err(_) => {
            let _ = std::process::Command::new("notify-send")
                .args(["-a", "grind", "grind", msg])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
    }
}

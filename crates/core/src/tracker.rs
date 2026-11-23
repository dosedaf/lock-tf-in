use anyhow::{bail, Result};
use chrono::{Local, NaiveDate};
use rusqlite::{params, OptionalExtension, Row};

use crate::db::{now_iso, today_str, Db};
use crate::model::{ProgressEntry, TrackerGroup, TrackerPayload, TrackerSubject, TrackerTotals};

pub const TRACKER_STATES: [&str; 5] = ["todo", "doing", "paused", "done", "dropped"];

pub fn kind_meta(kind: &str) -> Option<(&'static str, Option<f64>)> {
    Some(match kind {
        "book" => ("pages", None),
        "video" => ("min", None),
        "article" => ("%", Some(100.0)),
        "course" => ("lessons", None),
        "other" => ("units", None),
        _ => return None,
    })
}

fn subject_from_row(r: &Row<'_>) -> rusqlite::Result<TrackerSubject> {
    Ok(TrackerSubject {
        id: r.get("id")?,
        group_id: r.get("group_id")?,
        name: r.get("name")?,
        kind: r.get("kind")?,
        url: r.get("url")?,
        unit_label: r.get("unit_label")?,
        unit_total: r.get("unit_total")?,
        unit_done: r.get::<_, Option<f64>>("unit_done")?.unwrap_or(0.0),
        state: r.get("state")?,
        started_at: r.get("started_at")?,
        finished_at: r.get("finished_at")?,
        last_touched: r.get("last_touched")?,
        snooze_until: r.get("snooze_until")?,
        snooze_reason: r.get("snooze_reason")?,
        is_next: r.get::<_, i64>("is_next")? != 0,
        notes: r.get("notes")?,
        days_idle: None,
        stale: false,
        snoozed: false,
        focused_min: 0,
    })
}

impl Db {
    pub fn tracker_stale_days(&self) -> i64 {
        self.get_setting("tracker_stale_days")
            .and_then(|v| v.parse::<i64>().ok())
            .map(|n| n.clamp(1, 365))
            .unwrap_or(14)
    }

    pub fn tracker_focus(&self) -> String {
        self.get_setting_or("tracker_focus", "")
    }

    pub fn set_tracker_focus(&self, text: &str) -> Result<()> {
        let text: String = text.trim().chars().take(300).collect();
        self.set_setting("tracker_focus", &text)
    }

    pub fn set_tracker_stale_days(&self, n: i64) -> Result<()> {
        if !(1..=365).contains(&n) {
            bail!("stale_days must be 1..365");
        }
        self.set_setting("tracker_stale_days", &n.to_string())
    }

    pub fn touch_subject(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tracker_subjects SET last_touched=? WHERE id=?",
            params![now_iso(), id],
        )?;
        Ok(())
    }

    pub fn tracker_payload(&self) -> Result<TrackerPayload> {
        let today = Local::now().date_naive();
        let stale_days = self.tracker_stale_days();
        let mut focus = std::collections::HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT subject_id, SUM(minutes) FROM pomo_periods \
                 WHERE subject_id IS NOT NULL GROUP BY subject_id",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (id, min) = row?;
                focus.insert(id, min);
            }
        }

        let mut groups = Vec::new();
        let mut totals = TrackerTotals::default();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT id, name, position FROM tracker_groups ORDER BY position")?;
            let grows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?;
            for grow in grows {
                let (gid, name, position) = grow?;
                let mut subs = Vec::new();
                let mut sstmt = self.conn.prepare(
                    "SELECT * FROM tracker_subjects WHERE group_id=? ORDER BY position, id",
                )?;
                let srows = sstmt.query_map(params![gid], subject_from_row)?;
                for srow in srows {
                    let mut s = srow?;
                    s.focused_min = focus.get(&s.id).copied().unwrap_or(0);
                    if let Some(last) = s.last_touched.as_deref() {
                        if let Ok(d) = NaiveDate::parse_from_str(&last[..last.len().min(10)], "%Y-%m-%d")
                        {
                            s.days_idle = Some((today - d).num_days());
                        }
                    }
                    s.snoozed = s
                        .snooze_until
                        .as_deref()
                        .map(|u| u >= today_str().as_str())
                        .unwrap_or(false);
                    s.stale = s.state == "doing"
                        && s.days_idle.map(|d| d >= stale_days).unwrap_or(false)
                        && !s.snoozed;
                    subs.push(s);
                }
                let counted: Vec<&TrackerSubject> = subs
                    .iter()
                    .filter(|s| s.state != "dropped" && s.state != "paused")
                    .collect();
                let done_n = counted.iter().filter(|s| s.state == "done").count() as i64;
                for s in &subs {
                    if s.state == "doing" {
                        totals.active += 1;
                    }
                    if s.stale {
                        totals.stale += 1;
                    }
                    if s.state != "dropped" && s.state != "paused" {
                        if let Some(total) = s.unit_total {
                            totals.units_left += (total - s.unit_done).max(0.0);
                        }
                    }
                    totals.focused_min += s.focused_min;
                }
                let pct = if counted.is_empty() {
                    None
                } else {
                    Some((100.0 * done_n as f64 / counted.len() as f64).round() as i64)
                };
                groups.push(TrackerGroup {
                    id: gid,
                    name,
                    position,
                    pct,
                    done: done_n,
                    count: counted.len() as i64,
                    subjects: subs,
                });
            }
        }

        Ok(TrackerPayload {
            stale_days,
            focus: self.tracker_focus(),
            groups,
            totals,
        })
    }

    pub fn add_group(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "name required");
        let pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position),0)+1 FROM tracker_groups",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO tracker_groups (name, position) VALUES (?,?)",
            params![name, pos],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_group(&self, id: i64, name: &str) -> Result<()> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "name cannot be empty");
        let n = self
            .conn
            .execute("UPDATE tracker_groups SET name=? WHERE id=?", params![name, id])?;
        if n == 0 {
            bail!("group not found");
        }
        Ok(())
    }

    pub fn delete_group(&self, id: i64) -> Result<()> {
        let n = self
            .conn
            .execute("DELETE FROM tracker_groups WHERE id=?", params![id])?;
        if n == 0 {
            bail!("group not found");
        }
        Ok(())
    }

    pub fn reorder_groups(&self, order: &[i64]) -> Result<()> {
        anyhow::ensure!(!order.is_empty(), "order required");
        let tx = self.conn.unchecked_transaction()?;
        for (n, id) in order.iter().enumerate() {
            let changed = tx.execute(
                "UPDATE tracker_groups SET position=? WHERE id=?",
                params![n as i64 + 1, id],
            )?;
            anyhow::ensure!(changed == 1, "unknown group id");
        }
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_subject(
        &self,
        group_id: i64,
        name: &str,
        kind: &str,
        url: &str,
        notes: &str,
        unit_total: Option<f64>,
    ) -> Result<i64> {
        let exists = self
            .conn
            .query_row("SELECT 1 FROM tracker_groups WHERE id=?", params![group_id], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        if exists.is_none() {
            bail!("group not found");
        }
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "name required");
        let (label, default_total) = kind_meta(kind).ok_or_else(|| anyhow::anyhow!("bad kind"))?;
        let total = match unit_total {
            Some(t) if t > 0.0 => Some(t),
            Some(_) => bail!("unit_total must be a positive number"),
            None => default_total,
        };
        let pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position),0)+1 FROM tracker_subjects WHERE group_id=?",
            params![group_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO tracker_subjects \
             (group_id, name, kind, url, unit_total, unit_label, notes, position) \
             VALUES (?,?,?,?,?,?,?,?)",
            params![group_id, name, kind, url.trim(), total, label, notes.trim(), pos],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_subject(&self, id: i64) -> Result<()> {
        let n = self
            .conn
            .execute("DELETE FROM tracker_subjects WHERE id=?", params![id])?;
        if n == 0 {
            bail!("subject not found");
        }
        let mut st = self.pomo_state();
        if st.subject_id == Some(id) {
            st.subject_id = None;
        }
        if st.block_subject_id == Some(id) {
            st.block_subject_id = None;
        }
        self.set_setting("pomo_state", &serde_json::to_string(&st)?)?;
        Ok(())
    }

    pub fn subject_progress(
        &self,
        id: i64,
        delta: Option<f64>,
        set: Option<f64>,
        note: &str,
    ) -> Result<f64> {
        let (unit_done, unit_total): (f64, Option<f64>) = self
            .conn
            .query_row(
                "SELECT COALESCE(unit_done,0), unit_total FROM tracker_subjects WHERE id=?",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("subject not found"))?;
        let delta = match set {
            Some(target) => target - unit_done,
            None => delta.unwrap_or(0.0),
        };
        if delta == 0.0 {
            return Ok(unit_done);
        }
        let mut new_done = (unit_done + delta).max(0.0);
        if let Some(total) = unit_total {
            new_done = new_done.min(total);
        }
        let applied = ((new_done - unit_done) * 10_000.0).round() / 10_000.0;
        let now = now_iso();
        self.conn.execute(
            "UPDATE tracker_subjects SET unit_done=?, last_touched=?, \
             started_at=COALESCE(started_at, ?), \
             state=CASE WHEN state='todo' THEN 'doing' ELSE state END WHERE id=?",
            params![new_done, now, now, id],
        )?;
        self.conn.execute(
            "INSERT INTO tracker_progress_log (subject_id, logged_at, delta, unit_done_after, note) \
             VALUES (?,?,?,?,?)",
            params![id, now, applied, new_done, note.trim()],
        )?;
        Ok(new_done)
    }

    pub fn subject_state(&self, id: i64, state: &str) -> Result<()> {
        if !TRACKER_STATES.contains(&state) {
            bail!("invalid state");
        }
        let group_id: i64 = self
            .conn
            .query_row(
                "SELECT group_id FROM tracker_subjects WHERE id=?",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("subject not found"))?;
        let _ = group_id;
        let now = now_iso();
        self.conn.execute(
            "UPDATE tracker_subjects SET state=?, last_touched=?, \
             started_at=CASE WHEN ?='doing' THEN COALESCE(started_at, ?) ELSE started_at END, \
             finished_at=CASE WHEN ?='done' THEN COALESCE(finished_at, ?) ELSE NULL END \
             WHERE id=?",
            params![state, now, state, now, state, now, id],
        )?;
        Ok(())
    }

    pub fn set_subject_fields(
        &self,
        id: i64,
        name: Option<&str>,
        url: Option<&str>,
        notes: Option<&str>,
        kind: Option<&str>,
        unit_total: Option<Option<f64>>,
    ) -> Result<()> {
        let exists = self
            .conn
            .query_row("SELECT 1 FROM tracker_subjects WHERE id=?", params![id], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        if exists.is_none() {
            bail!("subject not found");
        }
        let mut sets: Vec<&str> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(name) = name {
            let name = name.trim();
            anyhow::ensure!(!name.is_empty(), "name cannot be empty");
            sets.push("name=?");
            values.push(name.to_string().into());
        }
        if let Some(url) = url {
            sets.push("url=?");
            values.push(url.trim().to_string().into());
        }
        if let Some(notes) = notes {
            sets.push("notes=?");
            values.push(notes.to_string().into());
        }
        if let Some(kind) = kind {
            let (label, _) = kind_meta(kind).ok_or_else(|| anyhow::anyhow!("bad kind"))?;
            sets.push("kind=?");
            values.push(kind.to_string().into());
            sets.push("unit_label=?");
            values.push(label.to_string().into());
        }
        if let Some(total) = unit_total {
            match total {
                Some(t) if t > 0.0 => {
                    sets.push("unit_total=?");
                    values.push(t.into());
                }
                Some(_) => bail!("unit_total must be a positive number"),
                None => {
                    sets.push("unit_total=?");
                    values.push(rusqlite::types::Value::Null);
                }
            }
        }
        anyhow::ensure!(!sets.is_empty(), "nothing to update");
        values.push(id.into());
        self.conn.execute(
            &format!("UPDATE tracker_subjects SET {} WHERE id=?", sets.join(", ")),
            rusqlite::params_from_iter(values),
        )?;
        Ok(())
    }

    pub fn tracker_set_next(&self, id: i64, on: bool) -> Result<()> {
        let group_id: i64 = self
            .conn
            .query_row(
                "SELECT group_id FROM tracker_subjects WHERE id=?",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("subject not found"))?;
        if on {
            self.conn.execute(
                "UPDATE tracker_subjects SET is_next=0 WHERE group_id=?",
                params![group_id],
            )?;
        }
        self.conn.execute(
            "UPDATE tracker_subjects SET is_next=? WHERE id=?",
            params![if on { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    pub fn snooze_subject(
        &self,
        id: i64,
        days: Option<i64>,
        until: Option<&str>,
        reason: &str,
        clear: bool,
    ) -> Result<()> {
        let exists = self
            .conn
            .query_row("SELECT 1 FROM tracker_subjects WHERE id=?", params![id], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        if exists.is_none() {
            bail!("subject not found");
        }
        if clear {
            self.conn.execute(
                "UPDATE tracker_subjects SET snooze_until=NULL, snooze_reason='' WHERE id=?",
                params![id],
            )?;
            return Ok(());
        }
        let until = if let Some(days) = days {
            if !(1..=365).contains(&days) {
                bail!("days must be 1..365");
            }
            (Local::now().date_naive() + chrono::Duration::days(days))
                .format("%Y-%m-%d")
                .to_string()
        } else {
            let u = until.unwrap_or("").trim();
            let d = NaiveDate::parse_from_str(u, "%Y-%m-%d")?;
            let delta = (d - Local::now().date_naive()).num_days();
            if !(0..=365).contains(&delta) {
                bail!("until must be a valid future date");
            }
            u.to_string()
        };
        self.conn.execute(
            "UPDATE tracker_subjects SET snooze_until=?, snooze_reason=? WHERE id=?",
            params![until, reason.trim(), id],
        )?;
        Ok(())
    }

    pub fn subject_history(&self, id: i64, limit: i64) -> Result<Vec<ProgressEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT logged_at, delta, unit_done_after, note FROM tracker_progress_log \
             WHERE subject_id=? ORDER BY id DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![id, limit], |r| {
            Ok(ProgressEntry {
                logged_at: r.get(0)?,
                delta: r.get(1)?,
                unit_done_after: r.get(2)?,
                note: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

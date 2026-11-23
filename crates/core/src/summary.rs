use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use rusqlite::params;
use rusqlite::OptionalExtension;

use crate::db::Db;
use crate::model::{Bucket, DailyCategory, Summary};

impl Db {
    pub fn summary(&self) -> Result<Summary> {
        self.summary_range(None)
    }

    pub fn summary_range(&self, range_days: Option<i64>) -> Result<Summary> {
        let today = Local::now().date_naive();
        let cutoff = range_days.map(|d| today - Duration::days(d.max(1) - 1));
        let mut rows = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT minutes, started_at FROM pomo_periods ORDER BY started_at")?;
            let iter = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
            for row in iter {
                rows.push(row?);
            }
        }

        let mut all_by_day: std::collections::BTreeMap<NaiveDate, i64> = Default::default();
        let mut by_day: std::collections::BTreeMap<NaiveDate, i64> = Default::default();
        let mut by_hour = [0i64; 24];
        let mut total_min = 0i64;
        let mut total_periods = 0i64;
        for (minutes, started) in &rows {
            if started.len() < 10 {
                continue;
            }
            let Ok(day) = NaiveDate::parse_from_str(&started[..10], "%Y-%m-%d") else {
                continue;
            };
            *all_by_day.entry(day).or_insert(0) += minutes;
            if cutoff.map(|c| day >= c).unwrap_or(true) {
                total_min += minutes;
                total_periods += 1;
                *by_day.entry(day).or_insert(0) += minutes;
                if started.len() >= 13 {
                    if let Ok(h) = started[11..13].parse::<usize>() {
                        if h < 24 {
                            by_hour[h] += minutes;
                        }
                    }
                }
            }
        }

        let dates: Vec<NaiveDate> = by_day.keys().copied().collect();
        let all_dates: Vec<NaiveDate> = all_by_day.keys().copied().collect();

        let mut best_streak = 0;
        let mut run = 0;
        let mut prev: Option<NaiveDate> = None;
        for d in &all_dates {
            run = match prev {
                Some(p) if (*d - p).num_days() == 1 => run + 1,
                _ => 1,
            };
            best_streak = best_streak.max(run);
            prev = Some(*d);
        }

        let mut cur_streak = 0;
        let mut d = if all_by_day.contains_key(&today) {
            today
        } else {
            today - Duration::days(1)
        };
        while all_by_day.contains_key(&d) {
            cur_streak += 1;
            d -= Duration::days(1);
        }

        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        let mut weeks = Vec::new();
        for i in (0..=7).rev() {
            let ws = monday - Duration::weeks(i as i64);
            let we = ws + Duration::days(7);
            let min: i64 = by_day
                .iter()
                .filter(|(day, _)| **day >= ws && **day < we)
                .map(|(_, v)| *v)
                .sum();
            weeks.push(Bucket {
                label: ws.format("%b %d").to_string(),
                min,
            });
        }

        let mstart = today.with_day(1).unwrap_or(today);
        let mut months = Vec::new();
        for i in (0..=11).rev() {
            let (y, mo) = month_back(mstart.year(), mstart.month(), i);
            let min: i64 = by_day
                .iter()
                .filter(|(day, _)| day.year() == y && day.month() == mo)
                .map(|(_, v)| *v)
                .sum();
            if min > 0 {
                months.push(Bucket {
                    label: month_name(mo).to_string(),
                    min,
                });
            }
        }

        let since = (today - Duration::days(6)).format("%Y-%m-%d").to_string();
        let mut daily_map: std::collections::BTreeMap<
            String,
            (std::collections::BTreeMap<String, i64>, i64),
        > = Default::default();
        {
            let mut stmt = self.conn.prepare(
                "SELECT substr(p.started_at,1,10) AS d, \
                 CASE WHEN p.subject_id IS NOT NULL THEN COALESCE(g.name, 'learning') \
                 ELSE COALESCE(c.name,'Unallocated') END AS bucket, SUM(p.minutes) AS m \
                 FROM pomo_periods p \
                 LEFT JOIN categories c ON c.id=p.category_id \
                 LEFT JOIN tracker_subjects s ON s.id=p.subject_id \
                 LEFT JOIN tracker_groups g ON g.id=s.group_id \
                 WHERE substr(p.started_at,1,10)>=? GROUP BY d, bucket",
            )?;
            let iter = stmt.query_map(params![since], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?;
            for row in iter {
                let (day, bucket, min) = row?;
                let entry = daily_map.entry(day).or_default();
                *entry.0.entry(bucket).or_insert(0) += min;
                entry.1 += min;
            }
        }

        let mut daily_categories = Vec::new();
        for i in (0..=6).rev() {
            let date = today - Duration::days(i);
            let key = date.format("%Y-%m-%d").to_string();
            let (cats, total) = daily_map.get(&key).cloned().unwrap_or_default();
            let mut cats: Vec<(String, i64)> = cats.into_iter().collect();
            cats.sort_by_key(|b| std::cmp::Reverse(b.1));
            daily_categories.push(DailyCategory {
                date: key,
                label: date.format("%a").to_string(),
                total,
                cats,
            });
        }

        let heat_days = range_days.unwrap_or(90).clamp(7, 180);
        let mut heatmap = Vec::new();
        for i in (0..heat_days).rev() {
            let date = today - Duration::days(i);
            heatmap.push(crate::model::HeatmapDay {
                date: date.format("%Y-%m-%d").to_string(),
                minutes: by_day.get(&date).copied().unwrap_or(0),
            });
        }

        Ok(Summary {
            total_min,
            total_periods,
            days_accessed: dates.len() as i64,
            streak_current: cur_streak,
            streak_best: best_streak,
            avg_per_focus_day: if dates.is_empty() {
                0
            } else {
                (total_min as f64 / dates.len() as f64).round() as i64
            },
            daily_categories,
            weeks,
            months,
            hours: by_hour.to_vec(),
            heatmap,
        })
    }

    pub fn periods(&self, days: i64) -> Result<Vec<crate::model::PeriodRow>> {
        let since = (Local::now().date_naive() - Duration::days(days))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.started_at, p.ended_at, p.minutes, p.category_id, p.subject_id, c.name, p.task_label, p.source \
             FROM pomo_periods p LEFT JOIN categories c ON c.id=p.category_id \
             WHERE substr(p.started_at,1,10)>=? ORDER BY p.started_at DESC LIMIT 500",
        )?;
        let rows = stmt.query_map(params![since], |r| {
            Ok(crate::model::PeriodRow {
                id: r.get(0)?,
                started_at: r.get(1)?,
                ended_at: r.get(2)?,
                minutes: r.get(3)?,
                category_id: r.get(4)?,
                subject_id: r.get(5)?,
                category_name: r.get(6)?,
                task_label: r.get(7)?,
                source: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn add_manual_period(
        &self,
        started_at: &str,
        ended_at: &str,
        minutes: i64,
        category_id: Option<i64>,
        subject_id: Option<i64>,
    ) -> Result<i64> {
        self.validate_period(started_at, ended_at, minutes, category_id, subject_id)?;
        self.conn.execute(
            "INSERT INTO pomo_periods \
             (session_id, day_id, category_id, subject_id, task_id, task_label, minutes, started_at, ended_at, source) \
             VALUES (NULL, NULL, ?, ?, NULL, '', ?, ?, ?, 'manual')",
            params![category_id, subject_id, minutes, started_at, ended_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_manual_period(
        &self,
        id: i64,
        started_at: &str,
        ended_at: &str,
        minutes: i64,
        category_id: Option<i64>,
        subject_id: Option<i64>,
    ) -> Result<()> {
        self.validate_period(started_at, ended_at, minutes, category_id, subject_id)?;
        let changed = self.conn.execute(
            "UPDATE pomo_periods SET category_id=?, subject_id=?, minutes=?, started_at=?, ended_at=? WHERE id=?",
            params![category_id, subject_id, minutes, started_at, ended_at, id],
        )?;
        anyhow::ensure!(changed > 0, "period not found");
        Ok(())
    }

    fn validate_period(
        &self,
        started_at: &str,
        ended_at: &str,
        minutes: i64,
        category_id: Option<i64>,
        subject_id: Option<i64>,
    ) -> Result<()> {
        let start =
            crate::db::parse_iso(started_at).ok_or_else(|| anyhow::anyhow!("invalid start"))?;
        let end = crate::db::parse_iso(ended_at).ok_or_else(|| anyhow::anyhow!("invalid end"))?;
        anyhow::ensure!(end > start, "end must be after start");
        anyhow::ensure!((1..=1440).contains(&minutes), "minutes must be 1..1440");
        if let Some(cat) = category_id {
            anyhow::ensure!(
                self.conn
                    .query_row("SELECT 1 FROM categories WHERE id=?", params![cat], |r| r
                        .get::<_, i64>(
                        0
                    ))
                    .optional()?
                    .is_some(),
                "category not found"
            );
        }
        if let Some(sub) = subject_id {
            anyhow::ensure!(
                self.conn
                    .query_row(
                        "SELECT 1 FROM tracker_subjects WHERE id=?",
                        params![sub],
                        |r| r.get::<_, i64>(0)
                    )
                    .optional()?
                    .is_some(),
                "subject not found"
            );
        }
        Ok(())
    }

    pub fn period_snapshot(&self, id: i64) -> Result<Option<crate::model::PeriodSnapshot>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, session_id, day_id, category_id, subject_id, task_id, task_label, minutes, started_at, ended_at, source \
                 FROM pomo_periods WHERE id=?",
                params![id],
                |r| {
                    Ok(crate::model::PeriodSnapshot {
                        id: r.get(0)?,
                        session_id: r.get(1)?,
                        day_id: r.get(2)?,
                        category_id: r.get(3)?,
                        subject_id: r.get(4)?,
                        task_id: r.get(5)?,
                        task_label: r.get(6)?,
                        minutes: r.get(7)?,
                        started_at: r.get(8)?,
                        ended_at: r.get(9)?,
                        source: r.get(10)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn restore_period(&self, p: &crate::model::PeriodSnapshot) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO pomo_periods \
             (id, session_id, day_id, category_id, subject_id, task_id, task_label, minutes, started_at, ended_at, source) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            params![
                p.id,
                p.session_id,
                p.day_id,
                p.category_id,
                p.subject_id,
                p.task_id,
                p.task_label,
                p.minutes,
                p.started_at,
                p.ended_at,
                p.source
            ],
        )?;
        Ok(())
    }

    pub fn delete_period(&self, id: i64) -> Result<()> {
        let n = self
            .conn
            .execute("DELETE FROM pomo_periods WHERE id=?", params![id])?;
        anyhow::ensure!(n > 0, "period not found");
        Ok(())
    }

    pub fn months(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT substr(started_at,1,7) AS m FROM pomo_periods ORDER BY m DESC",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out: Vec<String> = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        let cur = Local::now().format("%Y-%m").to_string();
        if !out.contains(&cur) {
            out.insert(0, cur);
        }
        Ok(out)
    }
}

fn month_back(mut year: i32, month: u32, back: i32) -> (i32, u32) {
    let mut m = month as i32 - back;
    while m <= 0 {
        m += 12;
        year -= 1;
    }
    (year, m as u32)
}

fn month_name(m: u32) -> &'static str {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    NAMES
        .get(m.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("")
}

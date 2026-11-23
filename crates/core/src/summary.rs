use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate};
use rusqlite::params;

use crate::db::{today_str, Db};
use crate::model::{Bucket, DailyCategory, Summary};

impl Db {
    pub fn summary(&self) -> Result<Summary> {
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
        let total_min: i64 = rows.iter().map(|(m, _)| *m).sum();
        let mut by_day: std::collections::BTreeMap<String, i64> = Default::default();
        let mut by_hour = [0i64; 24];
        for (minutes, started) in &rows {
            if started.len() >= 13 {
                by_day
                    .entry(started[..10].to_string())
                    .and_modify(|v| *v += minutes)
                    .or_insert(*minutes);
                if let Ok(h) = started[11..13].parse::<usize>() {
                    if h < 24 {
                        by_hour[h] += minutes;
                    }
                }
            }
        }

        let dates: Vec<NaiveDate> = by_day
            .keys()
            .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .collect();

        let mut best_streak = 0;
        let mut run = 0;
        let mut prev: Option<NaiveDate> = None;
        for d in &dates {
            run = match prev {
                Some(p) if (*d - p).num_days() == 1 => run + 1,
                _ => 1,
            };
            best_streak = best_streak.max(run);
            prev = Some(*d);
        }

        let today = Local::now().date_naive();
        let mut cur_streak = 0;
        let mut d = if by_day.contains_key(&today_str()) {
            today
        } else {
            today - Duration::days(1)
        };
        while by_day.contains_key(&d.format("%Y-%m-%d").to_string()) {
            cur_streak += 1;
            d -= Duration::days(1);
        }

        let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        let mut weeks = Vec::new();
        for i in (0..=7).rev() {
            let ws = monday - Duration::weeks(i as i64);
            let we = ws + Duration::days(7);
            let mut min = 0;
            for (day, v) in &by_day {
                if let Ok(day) = NaiveDate::parse_from_str(day, "%Y-%m-%d") {
                    if day >= ws && day < we {
                        min += v;
                    }
                }
            }
            weeks.push(Bucket {
                label: ws.format("%b %d").to_string(),
                min,
            });
        }

        let mstart = today.with_day(1).unwrap_or(today);
        let mut months = Vec::new();
        for i in (0..=11).rev() {
            let (y, mo) = month_back(mstart.year(), mstart.month(), i);
            let key = format!("{y:04}-{mo:02}");
            let min: i64 = by_day
                .iter()
                .filter(|(k, _)| k.starts_with(&key))
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

        Ok(Summary {
            total_min,
            total_periods: rows.len() as i64,
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
        })
    }

    pub fn periods(&self, days: i64) -> Result<Vec<crate::model::PeriodRow>> {
        let since = (Local::now().date_naive() - Duration::days(days))
            .format("%Y-%m-%d")
            .to_string();
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.started_at, p.ended_at, p.minutes, c.name, p.task_label, p.source \
             FROM pomo_periods p LEFT JOIN categories c ON c.id=p.category_id \
             WHERE substr(p.started_at,1,10)>=? ORDER BY p.started_at DESC LIMIT 500",
        )?;
        let rows = stmt.query_map(params![since], |r| {
            Ok(crate::model::PeriodRow {
                id: r.get(0)?,
                started_at: r.get(1)?,
                ended_at: r.get(2)?,
                minutes: r.get(3)?,
                category_name: r.get(4)?,
                task_label: r.get(5)?,
                source: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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

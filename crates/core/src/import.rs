use anyhow::Result;
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};
use rusqlite::params;

use crate::db::Db;
use crate::model::{ImportPreview, ImportResult};

#[derive(Default)]
struct Raw {
    date: Option<NaiveDate>,
    range: Option<(u32, u32, u32, u32)>,
    project: Option<String>,
    minutes: Option<i64>,
}

fn parse_date(line: &str) -> Option<NaiveDate> {
    let mut parts = line.split('-');
    let d = parts.next()?;
    let m = parts.next()?;
    let y = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    if !(1..=2).contains(&d.len()) || m.len() != 3 || y.len() != 4 {
        return None;
    }
    if !d.chars().all(|c| c.is_ascii_digit()) || !y.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    NaiveDate::parse_from_str(line, "%d-%b-%Y").ok()
}

fn parse_range(line: &str) -> Option<(u32, u32, u32, u32)> {
    let (a, b) = line.split_once('~')?;
    let parse = |s: &str| -> Option<(u32, u32)> {
        let s = s.trim();
        let (h, m) = s.split_once(':')?;
        if !(1..=2).contains(&h.len()) || m.len() != 2 {
            return None;
        }
        let h: u32 = h.parse().ok()?;
        let m: u32 = m.parse().ok()?;
        if h > 23 || m > 59 {
            return None;
        }
        Some((h, m))
    };
    let (sh, sm) = parse(a)?;
    let (eh, em) = parse(b)?;
    Some((sh, sm, eh, em))
}

pub fn parse_pomofocus(text: &str) -> Vec<ImportPreview> {
    let mut records = Vec::new();
    let mut cur: Option<Raw> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(date) = parse_date(line) {
            cur = Some(Raw {
                date: Some(date),
                ..Default::default()
            });
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        if c.range.is_none() {
            if let Some(range) = parse_range(line) {
                c.range = Some(range);
                continue;
            }
        }
        if c.range.is_some() && line.chars().all(|ch| ch.is_ascii_digit()) {
            if let Ok(minutes) = line.parse::<i64>() {
                let out = ImportPreview {
                    project: c
                        .project
                        .clone()
                        .filter(|p| !p.trim().is_empty())
                        .unwrap_or_else(|| "Unallocated".into()),
                    minutes,
                    start: String::new(),
                    end: String::new(),
                };
                let start = c.date.unwrap();
                records.push((out, start, c.range.unwrap()));
                cur = None;
                continue;
            }
        }
        if c.range.is_some() && c.project.is_none() {
            c.project = Some(line.to_string());
        }
    }

    let mut out = Vec::new();
    for (mut rec, date, (sh, sm, eh, em)) in records {
        let start = NaiveDateTime::new(
            date,
            NaiveTime::from_hms_opt(sh, sm, 0).unwrap_or_default(),
        );
        let mut end = NaiveDateTime::new(
            date,
            NaiveTime::from_hms_opt(eh, em, 0).unwrap_or_default(),
        );
        if end < start {
            end += Duration::days(1);
        }
        rec.start = start.format("%Y-%m-%dT%H:%M:%S").to_string();
        rec.end = end.format("%Y-%m-%dT%H:%M:%S").to_string();
        out.push(rec);
    }
    out
}

impl Db {
    pub fn ensure_category(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        let name = if name.is_empty() {
            "Unallocated"
        } else {
            name
        };
        self.conn.execute(
            "INSERT OR IGNORE INTO categories (name, position) \
             VALUES (?, COALESCE((SELECT MAX(position)+1 FROM categories),1))",
            params![name],
        )?;
        Ok(self.conn.query_row(
            "SELECT id FROM categories WHERE name=?",
            params![name],
            |r| r.get(0),
        )?)
    }

    pub fn import_pomofocus(&self, text: &str, dry_run: bool) -> Result<ImportResult> {
        let recs = parse_pomofocus(text);
        if dry_run {
            return Ok(ImportResult {
                parsed: recs.len(),
                imported: 0,
                skipped: 0,
                preview: recs.into_iter().take(50).collect(),
            });
        }
        let mut imported = 0usize;
        let mut skipped = 0usize;
        let mut cache: std::collections::HashMap<String, i64> = Default::default();
        let tx = self.conn.unchecked_transaction()?;
        for rec in &recs {
            let cat = if let Some(id) = cache.get(&rec.project) {
                *id
            } else {
                let name = if rec.project.trim().is_empty() {
                    "Unallocated"
                } else {
                    rec.project.as_str()
                };
                tx.execute(
                    "INSERT OR IGNORE INTO categories (name, position) \
                     VALUES (?, COALESCE((SELECT MAX(position)+1 FROM categories),1))",
                    params![name],
                )?;
                let id: i64 = tx.query_row(
                    "SELECT id FROM categories WHERE name=?",
                    params![name],
                    |r| r.get(0),
                )?;
                cache.insert(rec.project.clone(), id);
                id
            };
            let n = tx.execute(
                "INSERT OR IGNORE INTO pomo_periods \
                 (category_id, task_label, minutes, started_at, ended_at, source) \
                 VALUES (?,?,?,?,?, 'import')",
                params![cat, "", rec.minutes, rec.start, rec.end],
            )?;
            if n > 0 {
                imported += 1;
            } else {
                skipped += 1;
            }
        }
        tx.commit()?;
        Ok(ImportResult {
            parsed: recs.len(),
            imported,
            skipped,
            preview: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pomofocus_rows() {
        let text = "23-Sep-2026\n14:39 ~ 16:56\nclickhouse\n113\n\n22-Sep-2026\n18:22 ~ 20:00\nvideo related\n94";
        let rows = parse_pomofocus(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].project, "clickhouse");
        assert_eq!(rows[0].minutes, 113);
        assert_eq!(rows[0].start, "2026-09-23T14:39:00");
        assert_eq!(rows[0].end, "2026-09-23T16:56:00");
        assert_eq!(rows[1].project, "video related");
        assert_eq!(rows[1].minutes, 94);
    }

    #[test]
    fn crosses_midnight() {
        let text = "01-Jan-2026\n23:30 ~ 00:30\nlate\n60";
        let rows = parse_pomofocus(text);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].end, "2026-01-02T00:30:00");
    }
}

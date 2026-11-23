use anyhow::{Context, Result};
use chrono::{Local, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use crate::model::Task;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS days (
    id       INTEGER PRIMARY KEY,
    label    TEXT NOT NULL,
    position INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tasks (
    id       INTEGER PRIMARY KEY,
    day_id   INTEGER NOT NULL REFERENCES days(id) ON DELETE CASCADE,
    text     TEXT NOT NULL,
    details  TEXT NOT NULL DEFAULT '',
    state    TEXT NOT NULL DEFAULT 'todo' CHECK (state IN ('todo','doing','done')),
    position INTEGER NOT NULL,
    done_at  TEXT
);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT
);
CREATE TABLE IF NOT EXISTS categories (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS pomo_sessions (
    id          INTEGER PRIMARY KEY,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    tasks_done  INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS pomo_periods (
    id          INTEGER PRIMARY KEY,
    session_id  INTEGER REFERENCES pomo_sessions(id) ON DELETE SET NULL,
    day_id      INTEGER REFERENCES days(id) ON DELETE SET NULL,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    subject_id  INTEGER REFERENCES tracker_subjects(id) ON DELETE SET NULL,
    task_id     INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
    task_label  TEXT NOT NULL DEFAULT '',
    minutes     INTEGER NOT NULL,
    started_at  TEXT NOT NULL,
    ended_at    TEXT NOT NULL,
    source      TEXT NOT NULL DEFAULT 'timer'
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_periods_dedupe
    ON pomo_periods(category_id, task_label, minutes, started_at, ended_at);
CREATE TABLE IF NOT EXISTS tracker_groups (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    position INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tracker_subjects (
    id            INTEGER PRIMARY KEY,
    group_id      INTEGER NOT NULL REFERENCES tracker_groups(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL DEFAULT 'book' CHECK (kind IN ('book','video','article','course','other')),
    url           TEXT NOT NULL DEFAULT '',
    unit_total    REAL,
    unit_done     REAL NOT NULL DEFAULT 0,
    unit_label    TEXT NOT NULL DEFAULT 'pages',
    position_at   TEXT NOT NULL DEFAULT '',
    state         TEXT NOT NULL DEFAULT 'todo' CHECK (state IN ('todo','doing','paused','done','dropped')),
    started_at    TEXT,
    finished_at   TEXT,
    last_touched  TEXT,
    snooze_until  TEXT,
    snooze_reason TEXT NOT NULL DEFAULT '',
    is_next       INTEGER NOT NULL DEFAULT 0,
    notes         TEXT NOT NULL DEFAULT '',
    position      INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tracker_progress_log (
    id              INTEGER PRIMARY KEY,
    subject_id      INTEGER NOT NULL REFERENCES tracker_subjects(id) ON DELETE CASCADE,
    logged_at       TEXT NOT NULL,
    delta           REAL NOT NULL,
    unit_done_after REAL NOT NULL,
    note            TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_periods_started ON pomo_periods(started_at);
CREATE INDEX IF NOT EXISTS idx_periods_subject ON pomo_periods(subject_id);
CREATE INDEX IF NOT EXISTS idx_tasks_state ON tasks(state, position);
CREATE INDEX IF NOT EXISTS idx_subjects_group ON tracker_subjects(group_id, position);
CREATE INDEX IF NOT EXISTS idx_progress_subject ON tracker_progress_log(subject_id);
"#;

pub struct Db {
    pub(crate) conn: Connection,
    pub path: PathBuf,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_millis(4000))?;
        conn.execute_batch(SCHEMA)?;
        let db = Self {
            conn,
            path: path.to_path_buf(),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let has_col = |table: &str, col: &str| -> Result<bool> {
            let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let mut rows = stmt.query([])?;
            let mut found = false;
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                if name == col {
                    found = true;
                    break;
                }
            }
            Ok(found)
        };

        for (table, col, decl) in [
            ("tasks", "done_at", "TEXT"),
            ("pomo_sessions", "tasks_done", "INTEGER NOT NULL DEFAULT 0"),
            (
                "pomo_periods",
                "day_id",
                "INTEGER REFERENCES days(id) ON DELETE SET NULL",
            ),
            (
                "pomo_periods",
                "subject_id",
                "INTEGER REFERENCES tracker_subjects(id) ON DELETE SET NULL",
            ),
        ] {
            if !has_col(table, col)? {
                self.conn
                    .execute(&format!("ALTER TABLE {table} ADD COLUMN {col} {decl}"), [])?;
            }
        }

        if self
            .conn
            .query_row("SELECT COUNT(*) FROM days", [], |r| r.get::<_, i64>(0))?
            == 0
        {
            self.conn
                .execute("INSERT INTO days (label, position) VALUES ('Day 1', 1)", [])?;
            let day_id = self.conn.last_insert_rowid();
            self.set_setting("current_day_id", &day_id.to_string())?;
        }

        let day = self.current_day_id();
        self.conn.execute(
            "UPDATE pomo_periods SET day_id=? WHERE source='timer' AND day_id IS NULL",
            params![day],
        )?;
        self.conn.pragma_update(None, "user_version", 2)?;
        Ok(())
    }

    pub fn current_day_id(&self) -> i64 {
        if let Some(value) = self.get_setting("current_day_id") {
            if let Ok(id) = value.parse::<i64>() {
                let exists = self
                    .conn
                    .query_row("SELECT 1 FROM days WHERE id=?", params![id], |r| {
                        r.get::<_, i64>(0)
                    })
                    .optional()
                    .ok()
                    .flatten();
                if exists.is_some() {
                    return id;
                }
            }
        }
        self.conn
            .query_row("SELECT id FROM days ORDER BY position LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap_or(1)
    }

    pub fn create_day(&self, label: Option<&str>) -> Result<i64> {
        let pos: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(position),0)+1 FROM days", [], |r| {
                    r.get(0)
                })?;
        let label = label
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Day {pos}"));
        self.conn.execute(
            "INSERT INTO days (label, position) VALUES (?,?)",
            params![label, pos],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn days(&self) -> Result<Vec<(i64, String, i64)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, label, position FROM days ORDER BY position")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_setting(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key=?",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn get_setting_or(&self, key: &str, default: &str) -> String {
        self.get_setting(key).unwrap_or_else(|| default.to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key,value) VALUES (?,?) \
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn scratchpad(&self) -> String {
        self.get_setting_or("scratchpad", "")
    }

    pub fn set_scratchpad(&self, text: &str) -> Result<()> {
        let text: String = text.chars().take(20000).collect();
        self.set_setting("scratchpad", &text)
    }

    pub fn tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, day_id, text, details, state, position, done_at FROM tasks \
             ORDER BY CASE state WHEN 'doing' THEN 0 WHEN 'todo' THEN 1 ELSE 2 END, position",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Task {
                id: r.get(0)?,
                day_id: r.get(1)?,
                text: r.get(2)?,
                details: r.get(3)?,
                state: r.get(4)?,
                position: r.get(5)?,
                done_at: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn todo_stats(&self) -> Result<(i64, i64)> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(state='done'),0), COUNT(*) FROM tasks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    }

    pub fn add_task(&self, text: &str, details: &str) -> Result<i64> {
        let text = text.trim();
        anyhow::ensure!(!text.is_empty(), "text required");
        let pos: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(position),0)+1 FROM tasks", [], |r| {
                    r.get(0)
                })?;
        self.conn.execute(
            "INSERT INTO tasks (day_id, text, details, position) VALUES (?,?,?,?)",
            params![self.current_day_id(), text, details.trim(), pos],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn patch_task(
        &self,
        id: i64,
        text: Option<&str>,
        details: Option<&str>,
        state: Option<&str>,
    ) -> Result<()> {
        let exists = self
            .conn
            .query_row("SELECT 1 FROM tasks WHERE id=?", params![id], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        anyhow::ensure!(exists.is_some(), "task not found");
        let mut sets: Vec<&str> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(text) = text {
            let text = text.trim();
            anyhow::ensure!(!text.is_empty(), "text cannot be empty");
            sets.push("text=?");
            values.push(text.to_string().into());
        }
        if let Some(details) = details {
            sets.push("details=?");
            values.push(details.to_string().into());
        }
        if let Some(state) = state {
            anyhow::ensure!(
                matches!(state, "todo" | "doing" | "done"),
                "state must be todo|doing|done"
            );
            sets.push("state=?");
            values.push(state.to_string().into());
            if state == "done" {
                sets.push("done_at=COALESCE(done_at, ?)");
                values.push(now_iso().into());
            } else {
                sets.push("done_at=?");
                values.push(rusqlite::types::Value::Null);
            }
        }
        anyhow::ensure!(!sets.is_empty(), "nothing to update");
        values.push(id.into());
        self.conn.execute(
            &format!("UPDATE tasks SET {} WHERE id=?", sets.join(", ")),
            rusqlite::params_from_iter(values),
        )?;
        Ok(())
    }

    pub fn restore_task(&self, task: &Task) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO tasks (id, day_id, text, details, state, position, done_at) \
             VALUES (?,?,?,?,?,?,?)",
            params![
                task.id,
                task.day_id,
                task.text,
                task.details,
                task.state,
                task.position,
                task.done_at
            ],
        )?;
        Ok(())
    }

    pub fn task_by_id(&self, id: i64) -> Result<Option<Task>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, day_id, text, details, state, position, done_at FROM tasks WHERE id=?",
                params![id],
                |r| {
                    Ok(Task {
                        id: r.get(0)?,
                        day_id: r.get(1)?,
                        text: r.get(2)?,
                        details: r.get(3)?,
                        state: r.get(4)?,
                        position: r.get(5)?,
                        done_at: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn delete_task(&self, id: i64) -> Result<()> {
        let n = self
            .conn
            .execute("DELETE FROM tasks WHERE id=?", params![id])?;
        anyhow::ensure!(n > 0, "task not found");
        Ok(())
    }

    pub fn reorder_tasks(&self, order: &[i64]) -> Result<()> {
        if order.is_empty() {
            return Ok(());
        }
        let unique: std::collections::HashSet<_> = order.iter().collect();
        anyhow::ensure!(unique.len() == order.len(), "duplicate task ids");
        let base: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(position),0) FROM tasks", [], |r| {
                    r.get(0)
                })?;
        let tx = self.conn.unchecked_transaction()?;
        for (n, id) in order.iter().enumerate() {
            let changed = tx.execute(
                "UPDATE tasks SET position=? WHERE id=?",
                params![base + 1 + n as i64, id],
            )?;
            anyhow::ensure!(changed == 1, "unknown task id");
        }
        tx.commit()?;
        Ok(())
    }
}

pub fn read_setting(path: &Path, key: &str) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let conn = Connection::open(path).ok()?;
    conn.query_row(
        "SELECT value FROM settings WHERE key=?",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()
    .flatten()
}

pub fn now_iso() -> String {
    Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

pub fn today_str() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn parse_iso(s: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .or_else(|| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").ok())
}

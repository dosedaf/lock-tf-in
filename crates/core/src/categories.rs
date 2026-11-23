use anyhow::{bail, Result};
use rusqlite::{params, OptionalExtension};

use crate::db::Db;
use crate::model::Category;

impl Db {
    /// Categories with their all-time focused minutes (excludes the synthetic
    /// "Unallocated" row; the UI adds that itself).
    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, COALESCE(SUM(p.minutes),0) AS total_min \
             FROM categories c LEFT JOIN pomo_periods p ON p.category_id=c.id \
             GROUP BY c.id ORDER BY c.position",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                total_min: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn unallocated_minutes(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(minutes),0) FROM pomo_periods WHERE category_id IS NULL",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn add_category(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "name required");
        let exists = self
            .conn
            .query_row(
                "SELECT 1 FROM categories WHERE name=?",
                params![name],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        if exists.is_some() {
            bail!("category already exists");
        }
        let pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position),0)+1 FROM categories",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO categories (name, position) VALUES (?,?)",
            params![name, pos],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_category(&self, id: i64, name: &str) -> Result<()> {
        let name = name.trim();
        anyhow::ensure!(!name.is_empty(), "name cannot be empty");
        let changed = self
            .conn
            .execute("UPDATE categories SET name=? WHERE id=?", params![name, id])?;
        if changed == 0 {
            bail!("category not found");
        }
        Ok(())
    }

    pub fn delete_category(&self, id: i64) -> Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM categories WHERE id=?", params![id])?;
        if changed == 0 {
            bail!("category not found");
        }
        let mut st = self.pomo_state();
        if st.category_id == Some(id) {
            st.category_id = None;
        }
        if st.block_category_id == Some(id) {
            st.block_category_id = None;
        }
        self.set_setting("pomo_state", &serde_json::to_string(&st)?)?;
        Ok(())
    }
}

impl Db {
    pub fn merge_category(&self, from: i64, to: i64) -> Result<()> {
        anyhow::ensure!(from != to, "cannot merge a category into itself");
        for id in [from, to] {
            let exists = self
                .conn
                .query_row("SELECT 1 FROM categories WHERE id=?", params![id], |r| {
                    r.get::<_, i64>(0)
                })
                .optional()?;
            anyhow::ensure!(exists.is_some(), "category not found");
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE pomo_periods SET category_id=? WHERE category_id=?",
            params![to, from],
        )?;
        tx.execute(
            "UPDATE pomo_sessions SET category_id=? WHERE category_id=?",
            params![to, from],
        )?;
        tx.execute("DELETE FROM categories WHERE id=?", params![from])?;
        tx.commit()?;

        let mut st = self.pomo_state();
        if st.category_id == Some(from) {
            st.category_id = Some(to);
        }
        if st.block_category_id == Some(from) {
            st.block_category_id = Some(to);
        }
        self.set_setting("pomo_state", &serde_json::to_string(&st)?)?;
        Ok(())
    }

    pub fn assign_unallocated(&self, to: i64) -> Result<usize> {
        let exists = self
            .conn
            .query_row("SELECT 1 FROM categories WHERE id=?", params![to], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?;
        anyhow::ensure!(exists.is_some(), "category not found");
        let n = self.conn.execute(
            "UPDATE pomo_periods SET category_id=? WHERE category_id IS NULL",
            params![to],
        )?;
        Ok(n)
    }
}

use std::path::PathBuf;

use chrono::{Duration as ChronoDuration, Local};
use serde_json::json;

use crate::{Db, PomoAction};

fn temp_db(name: &str) -> (Db, PathBuf) {
    let path = std::env::temp_dir().join(format!("lockin-test-{}-{}.db", name, std::process::id()));
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
    let db = Db::open(&path).expect("open temp db");
    (db, path)
}

fn cleanup(path: &std::path::Path) {
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{}", path.display(), suffix));
    }
}

#[test]
fn tasks_crud_state_and_order() {
    let (db, path) = temp_db("tasks");
    let a = db.add_task("alpha", "").unwrap();
    let b = db.add_task("beta", "notes").unwrap();
    assert_eq!(db.tasks().unwrap().len(), 2);
    assert_eq!(db.todo_stats().unwrap(), (0, 2));

    db.patch_task(a, None, None, Some("done")).unwrap();
    assert_eq!(db.todo_stats().unwrap(), (1, 2));
    assert!(db
        .tasks()
        .unwrap()
        .iter()
        .any(|t| t.id == a && t.state == "done"));

    db.patch_task(a, None, None, Some("todo")).unwrap();
    assert_eq!(db.todo_stats().unwrap(), (0, 2));

    db.patch_task(b, Some("beta edited"), Some("details"), None)
        .unwrap();
    let beta = db.tasks().unwrap().into_iter().find(|t| t.id == b).unwrap();
    assert_eq!(beta.text, "beta edited");
    assert_eq!(beta.details, "details");

    db.reorder_tasks(&[b, a]).unwrap();
    let ids: Vec<i64> = db.tasks().unwrap().into_iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![b, a]);

    db.delete_task(a).unwrap();
    assert_eq!(db.todo_stats().unwrap(), (0, 1));
    drop(db);
    cleanup(&path);
}

#[test]
fn pomodoro_logs_finished_focus_and_advances() {
    let (db, path) = temp_db("pomo");
    db.set_setting("pomo_focus_min", "1").unwrap();
    db.set_setting("pomo_break_min", "1").unwrap();
    let view = db.pomo_action(PomoAction::Start).unwrap();
    assert!(view.running);

    let mut state = db.pomo_state();
    let past = (Local::now().naive_local() - ChronoDuration::minutes(2))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    state.started_at = Some(past.clone());
    state.phase_started_at = Some(past);
    db.set_setting("pomo_state", &serde_json::to_string(&state).unwrap())
        .unwrap();

    let view = db.pomo_tick().unwrap();
    assert_eq!(view.phase, "break");
    assert!(!view.running);

    let periods = db.periods(2).unwrap();
    assert!(periods
        .iter()
        .any(|p| p.minutes >= 1 && p.source == "timer"));
    drop(db);
    cleanup(&path);
}

#[test]
fn tracker_progress_state_reorder_and_snooze() {
    let (db, path) = temp_db("tracker");
    let g = db.add_group("group").unwrap();
    let s1 = db
        .add_subject(g, "one", "book", "", "", Some(100.0))
        .unwrap();
    let s2 = db.add_subject(g, "two", "video", "", "", None).unwrap();

    let done = db.subject_progress(s1, Some(25.0), None, "pages").unwrap();
    assert!((done - 25.0).abs() < 0.001);
    let payload = db.tracker_payload().unwrap();
    let subject = payload
        .groups
        .iter()
        .flat_map(|g| g.subjects.iter())
        .find(|s| s.id == s1)
        .unwrap();
    assert!((subject.unit_done - 25.0).abs() < 0.001);
    assert_eq!(subject.state, "doing");

    db.subject_state(s1, "done").unwrap();
    db.reorder_subjects(&[s2, s1]).unwrap();
    let payload = db.tracker_payload().unwrap();
    let ids: Vec<i64> = payload.groups[0].subjects.iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![s2, s1]);

    db.snooze_subject(s1, Some(7), None, "later", false)
        .unwrap();
    let subject = db
        .tracker_payload()
        .unwrap()
        .groups
        .into_iter()
        .flat_map(|g| g.subjects)
        .find(|s| s.id == s1)
        .unwrap();
    assert!(subject.snoozed);
    assert_eq!(subject.snooze_reason, "later");
    db.snooze_subject(s1, None, None, "", true).unwrap();

    db.reorder_groups(&[g]).unwrap();
    drop(db);
    cleanup(&path);
}

#[test]
fn settings_roundtrip_and_read_only_peek() {
    let (db, path) = temp_db("settings");
    db.set_setting("ui_zoom", "1.30").unwrap();
    db.set_setting("ui_always_on_top", "1").unwrap();
    assert_eq!(db.get_setting("ui_zoom").as_deref(), Some("1.30"));
    assert_eq!(
        crate::read_setting(&path, "ui_always_on_top").as_deref(),
        Some("1")
    );
    let raw = serde_json::to_string(&json!({"ok": true})).unwrap();
    db.set_setting("probe", &raw).unwrap();
    assert!(db.get_setting("probe").unwrap().contains("true"));
    drop(db);
    cleanup(&path);
}

#[test]
fn import_summary_and_card_render() {
    let (db, path) = temp_db("summary-card");
    let text = "23-Sep-2026\n14:39 ~ 16:56\nclickhouse\n113\n";
    let imported = db.import_pomofocus(text, false).unwrap();
    assert_eq!(imported.imported, 1);
    let summary = db.summary().unwrap();
    assert_eq!(summary.total_min, 113);
    assert_eq!(summary.total_periods, 1);
    let periods = db.periods(365).unwrap();
    assert!(periods
        .iter()
        .any(|p| p.minutes == 113 && p.category_name.as_deref() == Some("clickhouse")));
    let png = db.card_png(&crate::CardKind::Day).unwrap();
    assert!(png.len() > 1000);
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    drop(db);
    cleanup(&path);
}

#[test]
fn groups_reorder() {
    let (db, path) = temp_db("groups");
    let a = db.add_group("a").unwrap();
    let b = db.add_group("b").unwrap();
    db.reorder_groups(&[b, a]).unwrap();
    let ids: Vec<i64> = db
        .tracker_payload()
        .unwrap()
        .groups
        .iter()
        .map(|g| g.id)
        .collect();
    assert_eq!(ids, vec![b, a]);
    drop(db);
    cleanup(&path);
}

#[test]
fn manual_period_add_update_and_summary() {
    let (db, path) = temp_db("manual-period");
    let cat = db.add_category("work").unwrap();
    let group = db.add_group("g").unwrap();
    let sub = db
        .add_subject(group, "subject", "other", "", "", None)
        .unwrap();
    let id = db
        .add_manual_period(
            "2026-11-22T14:00:00",
            "2026-11-22T18:00:00",
            240,
            Some(cat),
            None,
        )
        .unwrap();
    let p = db
        .periods(365)
        .unwrap()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap();
    assert_eq!(p.minutes, 240);
    assert_eq!(p.category_name.as_deref(), Some("work"));
    assert_eq!(p.category_id, Some(cat));
    assert_eq!(p.subject_id, None);

    db.update_manual_period(
        id,
        "2026-11-22T13:00:00",
        "2026-11-22T15:30:00",
        150,
        Some(cat),
        Some(sub),
    )
    .unwrap();
    let p = db
        .periods(365)
        .unwrap()
        .into_iter()
        .find(|p| p.id == id)
        .unwrap();
    assert_eq!(p.minutes, 150);
    assert_eq!(p.subject_id, Some(sub));
    assert_eq!(db.summary().unwrap().total_min, 150);

    assert!(db
        .add_manual_period("2026-11-22T14:00:00", "2026-11-22T13:00:00", 60, None, None)
        .is_err());
    assert!(db
        .add_manual_period("2026-11-22T14:00:00", "2026-11-22T15:00:00", 0, None, None)
        .is_err());
    drop(db);
    cleanup(&path);
}

#[test]
fn undo_snapshots_restore_task_period_and_tracker() {
    let (db, path) = temp_db("undo");
    let t = db.add_task("undo me", "details").unwrap();
    let task = db.tasks().unwrap().into_iter().find(|x| x.id == t).unwrap();
    db.delete_task(t).unwrap();
    assert!(db.task_by_id(t).unwrap().is_none());
    db.restore_task(&task).unwrap();
    let restored = db.task_by_id(t).unwrap().unwrap();
    assert_eq!(restored.text, "undo me");
    assert_eq!(restored.details, "details");

    let cat = db.add_category("work").unwrap();
    let p = db
        .add_manual_period(
            "2026-11-22T14:00:00",
            "2026-11-22T15:00:00",
            60,
            Some(cat),
            None,
        )
        .unwrap();
    let snapshot = db.period_snapshot(p).unwrap().unwrap();
    db.delete_period(p).unwrap();
    assert!(db.period_snapshot(p).unwrap().is_none());
    db.restore_period(&snapshot).unwrap();
    assert_eq!(db.period_snapshot(p).unwrap().unwrap().minutes, 60);

    let g = db.add_group("g").unwrap();
    let s = db.add_subject(g, "s", "other", "", "", None).unwrap();
    db.subject_progress(s, Some(5.0), None, "").unwrap();
    let before = db
        .tracker_payload()
        .unwrap()
        .groups
        .into_iter()
        .flat_map(|g| g.subjects)
        .find(|x| x.id == s)
        .unwrap();
    let log_id = db.last_progress_log_id(s).unwrap();
    db.subject_state(s, "done").unwrap();
    db.restore_subject_state(
        s,
        "doing",
        before.started_at.as_deref(),
        None,
        before.last_touched.as_deref(),
    )
    .unwrap();
    let current = db
        .tracker_payload()
        .unwrap()
        .groups
        .into_iter()
        .flat_map(|g| g.subjects)
        .find(|x| x.id == s)
        .unwrap();
    assert_eq!(current.state, "doing");
    db.restore_subject_progress(s, before.unit_done, &before.state, log_id)
        .unwrap();
    let current = db
        .tracker_payload()
        .unwrap()
        .groups
        .into_iter()
        .flat_map(|g| g.subjects)
        .find(|x| x.id == s)
        .unwrap();
    assert!((current.unit_done - before.unit_done).abs() < 0.001);
    drop(db);
    cleanup(&path);
}

#[test]
fn category_merge_and_unallocated_cleanup() {
    let (db, path) = temp_db("categories");
    let a = db.add_category("old").unwrap();
    let b = db.add_category("new").unwrap();
    let p1 = db
        .add_manual_period(
            "2026-11-22T10:00:00",
            "2026-11-22T11:00:00",
            60,
            Some(a),
            None,
        )
        .unwrap();
    let p2 = db
        .add_manual_period("2026-11-22T11:00:00", "2026-11-22T12:00:00", 60, None, None)
        .unwrap();
    db.merge_category(a, b).unwrap();
    assert!(
        db.periods(365)
            .unwrap()
            .into_iter()
            .find(|p| p.id == p1)
            .unwrap()
            .category_id
            == Some(b)
    );
    assert_eq!(db.unallocated_minutes().unwrap(), 60);
    assert_eq!(db.assign_unallocated(b).unwrap(), 1);
    assert_eq!(db.unallocated_minutes().unwrap(), 0);
    let p2 = db
        .periods(365)
        .unwrap()
        .into_iter()
        .find(|p| p.id == p2)
        .unwrap();
    assert_eq!(p2.category_id, Some(b));
    drop(db);
    cleanup(&path);
}

#[test]
fn summary_range_and_heatmap() {
    let (db, path) = temp_db("summary-range");
    db.import_pomofocus("23-Sep-2026\n14:39 ~ 16:56\nclickhouse\n113\n", false)
        .unwrap();
    let all = db.summary().unwrap();
    assert_eq!(all.total_min, 113);
    assert_eq!(all.heatmap.len(), 90);
    assert_eq!(all.hours[14], 113);
    let recent = db.summary_range(Some(7)).unwrap();
    assert_eq!(recent.total_min, 0);
    assert_eq!(recent.heatmap.len(), 7);
    drop(db);
    cleanup(&path);
}

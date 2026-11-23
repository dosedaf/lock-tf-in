use anyhow::{bail, Result};
use chrono::{Duration, Local, NaiveDate, NaiveTime};
use lockin_core::{Db, PomoAction};
use std::path::PathBuf;

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

fn default_db() -> PathBuf {
    if let Ok(path) = std::env::var("LOCKIN_DB") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    let repo = home().join("projects/obs-todo/todo.db");
    if repo.exists() {
        return repo;
    }
    home().join(".local/share/lockin/todo.db")
}

fn hm(minutes: i64) -> String {
    if minutes == 0 {
        return "0m".into();
    }
    let h = minutes / 60;
    let m = minutes % 60;
    match (h, m) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn main() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    if let Err(e) = run() {
        eprintln!("lockinctl: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut db_path = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--db" && i + 1 < args.len() {
            db_path = Some(PathBuf::from(args.remove(i + 1)));
            args.remove(i);
            continue;
        }
        i += 1;
    }
    let db = Db::open(&db_path.unwrap_or_else(default_db))?;
    let Some(cmd) = args.first().cloned() else {
        print_help();
        return Ok(());
    };
    match cmd.as_str() {
        "task" | "tasks" => task_cmd(&db, &args[1..]),
        "timer" => timer_cmd(&db, &args[1..]),
        "log" => log_cmd(&db, &args[1..]),
        "tracker" => tracker_cmd(&db, &args[1..]),
        "summary" => summary_cmd(&db, &args[1..]),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => bail!("unknown command {other:?} (try `lockinctl help`)"),
    }
}

fn task_cmd(db: &Db, args: &[String]) -> Result<()> {
    let Some(sub) = args.first().map(String::as_str) else {
        bail!("task add|done|todo|doing|list");
    };
    match sub {
        "add" => {
            let text = args[1..].join(" ");
            let id = db.add_task(&text, "")?;
            println!("task {id} added");
        }
        "done" | "todo" | "doing" => {
            let id: i64 = args.get(1).context("task id")?.parse()?;
            db.patch_task(id, None, None, Some(sub))?;
            println!("task {id} -> {sub}");
        }
        "list" => {
            let (done, total) = db.todo_stats()?;
            println!("{done}/{total} done");
            for t in db.tasks()? {
                println!("{:>5}  {:<7}  {}", t.id, t.state, t.text);
            }
        }
        _ => bail!("task add|done|todo|doing|list"),
    }
    Ok(())
}

fn timer_cmd(db: &Db, args: &[String]) -> Result<()> {
    let Some(sub) = args.first().map(String::as_str) else {
        bail!("timer start|pause|reset|skip|status");
    };
    if sub == "status" {
        let v = db.pomo_view()?;
        let s = v.remaining_ms / 1000;
        println!(
            "{} {}:{:02}:{:02} {}{}",
            v.phase,
            s / 3600,
            (s / 60) % 60,
            s % 60,
            if v.running { "running" } else { "paused" },
            if v.ended { " ended" } else { "" }
        );
        println!(
            "today {} · {} periods",
            hm(v.today_focused_min),
            v.today_periods
        );
        return Ok(());
    }
    let action = match sub {
        "start" => PomoAction::Start,
        "pause" => PomoAction::Pause,
        "reset" => PomoAction::Reset,
        "skip" => PomoAction::Skip,
        _ => bail!("timer start|pause|reset|skip|status"),
    };
    let v = db.pomo_action(action)?;
    let s = v.remaining_ms / 1000;
    println!(
        "{} {}:{:02}:{:02} {}",
        v.phase,
        s / 3600,
        (s / 60) % 60,
        s % 60,
        if v.running { "running" } else { "paused" }
    );
    Ok(())
}

fn log_cmd(db: &Db, args: &[String]) -> Result<()> {
    let minutes: i64 = args
        .first()
        .context("minutes, e.g. `lockinctl log 120 --category work`")?
        .parse()?;
    if !(1..=1440).contains(&minutes) {
        bail!("minutes must be 1..1440");
    }
    let mut category = None;
    let mut subject = None;
    let mut date = Local::now().date_naive();
    let mut start_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--category" | "-c" => {
                category = args.get(i + 1).cloned();
                i += 1;
            }
            "--subject" | "-s" => {
                subject = args.get(i + 1).cloned().and_then(|s| s.parse::<i64>().ok());
                i += 1;
            }
            "--date" | "-d" => {
                date = NaiveDate::parse_from_str(args.get(i + 1).context("date")?, "%Y-%m-%d")?;
                i += 1;
            }
            "--start" => {
                start_time = NaiveTime::parse_from_str(args.get(i + 1).context("start")?, "%H:%M")?;
                i += 1;
            }
            other => bail!("unknown flag {other}"),
        }
        i += 1;
    }
    let cat_id = match category {
        Some(name) => Some(db.ensure_category(&name)?),
        None => None,
    };
    let start = date.and_time(start_time);
    let end = start + Duration::minutes(minutes);
    let started_at = start.format("%Y-%m-%dT%H:%M:%S").to_string();
    let ended_at = end.format("%Y-%m-%dT%H:%M:%S").to_string();
    let id = db.add_manual_period(&started_at, &ended_at, minutes, cat_id, subject)?;
    println!("period {id}: {started_at} → {ended_at} ({minutes}m)");
    Ok(())
}

fn tracker_cmd(db: &Db, args: &[String]) -> Result<()> {
    let Some(sub) = args.first().map(String::as_str) else {
        bail!("tracker progress|state|list");
    };
    match sub {
        "progress" => {
            let id: i64 = args.get(1).context("subject id")?.parse()?;
            let amount = args.get(2).context("delta or set:value")?;
            if let Some(value) = amount.strip_prefix("set:") {
                let v: f64 = value.parse()?;
                let done = db.subject_progress(id, None, Some(v), "cli")?;
                println!("subject {id} = {done}");
            } else {
                let delta: f64 = amount.parse()?;
                let done = db.subject_progress(id, Some(delta), None, "cli")?;
                println!("subject {id} = {done}");
            }
        }
        "state" => {
            let id: i64 = args.get(1).context("subject id")?.parse()?;
            let state = args.get(2).context("state")?;
            db.subject_state(id, state)?;
            println!("subject {id} -> {state}");
        }
        "list" => {
            for g in db.tracker_payload()?.groups {
                println!("{} ({}%)", g.name, g.pct.unwrap_or(0));
                for s in g.subjects {
                    println!("  {:>5}  {:<7}  {}", s.id, s.state, s.name);
                }
            }
        }
        _ => bail!("tracker progress|state|list"),
    }
    Ok(())
}

fn summary_cmd(db: &Db, args: &[String]) -> Result<()> {
    let days = if let Some(pos) = args.iter().position(|a| a == "--days") {
        args.get(pos + 1).and_then(|s| s.parse::<i64>().ok())
    } else {
        None
    };
    let s = db.summary_range(days)?;
    println!(
        "{} focused · {} total · {} avg/day · {} day streak",
        hm(s.total_min),
        s.total_periods,
        hm(s.avg_per_focus_day),
        s.streak_current
    );
    for day in &s.daily_categories {
        if day.total > 0 {
            let cats = day
                .cats
                .iter()
                .map(|(n, m)| format!("{n} {}", hm(*m)))
                .collect::<Vec<_>>()
                .join(", ");
            println!("{}  {}  ({cats})", day.label, hm(day.total));
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "lockinctl [--db PATH] <command>

templates
  task add <text>
  task done|todo|doing <id>
  task list
  timer start|pause|reset|skip|status
  log <minutes> [--category NAME] [--subject ID] [--date YYYY-MM-DD] [--start HH:MM]
  tracker progress <id> <delta|set:value>
  tracker state <id> <todo|doing|paused|done|dropped>
  tracker list
  summary [--days N]"
    );
}

use anyhow::Context as _;

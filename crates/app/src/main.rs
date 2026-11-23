mod app;
mod theme;
mod views;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use eframe::egui;

fn resolve_db(explicit: Option<String>) -> PathBuf {
    if let Some(path) = explicit {
        return PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("LOCKIN_DB") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Some(path) = dirs::home_dir().map(|h| h.join("projects/obs-todo/todo.db")) {
        if path.exists() {
            return path;
        }
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lockin/todo.db")
}

fn parse_args() -> (Option<String>, bool) {
    let mut db = None;
    let mut quit_existing = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db = args.next(),
            "--quit" => quit_existing = true,
            "--help" | "-h" => {
                println!("lockin [--db PATH] [--quit]");
                std::process::exit(0);
            }
            other => {
                if let Some(path) = other.strip_prefix("--db=") {
                    db = Some(path.to_string());
                }
            }
        }
    }
    (db, quit_existing)
}

fn socket_path() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    Path::new(&runtime).join("lockin-app.sock")
}

/// Try to hand off to an already running instance. Returns true if it did.
fn hand_off(quit_existing: bool) -> bool {
    let path = socket_path();
    if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) {
        let msg = if quit_existing { b"quit" as &[u8] } else { b"show" };
        let _ = stream.write_all(msg);
        return true;
    }
    false
}

/// Listen for later launches; they raise or quit this instance.
fn listen_single_instance(ctx: &egui::Context) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = std::os::unix::net::UnixListener::bind(&path) else {
        return;
    };
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut buf = [0u8; 8];
            let mut stream = stream;
            let n = stream.read(&mut buf).unwrap_or(0);
            if &buf[..n] == b"quit" {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            ctx.request_repaint();
        }
    });
}

fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("../../../assets/img/icon.png");
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            egui::IconData {
                rgba: rgba.as_raw().clone(),
                width: rgba.width(),
                height: rgba.height(),
            }
        }
        Err(_) => egui::IconData {
            rgba: vec![0, 0, 0, 255],
            width: 1,
            height: 1,
        },
    }
}

fn main() -> eframe::Result {
    let (db_arg, quit_existing) = parse_args();
    let db_path = resolve_db(db_arg);

    if hand_off(quit_existing) {
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LOCK IN")
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([860.0, 600.0])
            .with_app_id("org.lockin.app")
            .with_icon(app_icon()),
        ..Default::default()
    };

    let db_for_app = db_path.clone();
    eframe::run_native(
        "LOCK IN",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);
            listen_single_instance(&cc.egui_ctx);
            lockin_core::spawn_ticker(db_for_app.clone());
            Ok(Box::new(app::App::new(&cc.egui_ctx, db_for_app)))
        }),
    )
}

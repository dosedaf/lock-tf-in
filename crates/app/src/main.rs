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

fn parse_args() -> (Option<String>, bool, bool) {
    let mut db = None;
    let mut quit_existing = false;
    let mut reset_window = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => db = args.next(),
            "--quit" => quit_existing = true,
            "--reset-window" => reset_window = true,
            "--help" | "-h" => {
                println!("lockin [--db PATH] [--quit] [--reset-window]");
                std::process::exit(0);
            }
            other => {
                if let Some(path) = other.strip_prefix("--db=") {
                    db = Some(path.to_string());
                }
            }
        }
    }
    (db, quit_existing, reset_window)
}

fn socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("LOCKIN_SOCKET") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    Path::new(&runtime).join("lockin-app.sock")
}

pub fn pid_path() -> PathBuf {
    socket_path().with_extension("pid")
}

/// Try to hand off to an already running instance. Returns true if it did.
fn stop_running() {
    if let Ok(pid) = std::fs::read_to_string(pid_path()) {
        let pid = pid.trim().to_string();
        if !pid.is_empty() && Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::process::Command::new("kill").arg(&pid).status();
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }
    let _ = std::fs::remove_file(socket_path());
    let _ = std::fs::remove_file(pid_path());
}

fn hand_off(quit_existing: bool) -> bool {
    let path = socket_path();
    if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(&path) {
        let msg = if quit_existing {
            b"quit" as &[u8]
        } else {
            b"show"
        };
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
                app::QUIT_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed);
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
    // Always-on-top is a no-op under GNOME Wayland. XWayland supports it,
    // so use X11 when a display is available; LOCKIN_WAYLAND=1 opts back in.
    if std::env::var_os("DISPLAY").is_some()
        && std::env::var("LOCKIN_WAYLAND").ok().as_deref() != Some("1")
    {
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    let (db_arg, quit_existing, reset_window) = parse_args();
    let db_path = resolve_db(db_arg);

    if reset_window {
        if let Ok(db) = lockin_core::Db::open(&db_path) {
            let _ = db.set_setting("ui_window_w", "960");
            let _ = db.set_setting("ui_window_h", "780");
        }
        if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(socket_path()) {
            let _ = stream.write_all(b"quit");
        }
        std::thread::sleep(std::time::Duration::from_millis(700));
        stop_running();
    } else if hand_off(quit_existing) {
        if quit_existing {
            std::thread::sleep(std::time::Duration::from_millis(700));
            stop_running();
        }
        return Ok(());
    }

    let win_w = lockin_core::read_setting(&db_path, "ui_window_w")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(960.0)
        .max(1.0);
    let win_h = lockin_core::read_setting(&db_path, "ui_window_h")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(780.0)
        .max(1.0);

    let always_on_top =
        lockin_core::read_setting(&db_path, "ui_always_on_top").as_deref() == Some("1");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LOCK IN")
            .with_inner_size([win_w, win_h])
            .with_window_level(if always_on_top {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            })
            .with_resizable(true)
            .with_maximized(false)
            .with_decorations(false)
            .with_app_id("org.lockin.app")
            .with_icon(app_icon()),
        ..Default::default()
    };

    let _ = std::fs::write(pid_path(), std::process::id().to_string());
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

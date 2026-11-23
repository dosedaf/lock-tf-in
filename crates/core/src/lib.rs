pub mod card;
pub mod categories;
pub mod db;
pub mod import;
pub mod model;
pub mod pomodoro;
pub mod summary;
pub mod tracker;

pub use db::{read_setting, Db};
pub use model::*;
pub use pomodoro::{notify_phase, PomoAction};

use std::path::PathBuf;

/// Finalize due pomodoro phases even while the window is hidden or idle.
/// This is the only background activity in the app: one cheap query a second.
pub fn spawn_ticker(db_path: PathBuf) {
    std::thread::Builder::new()
        .name("pomo-ticker".into())
        .spawn(move || {
            let db = match Db::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("lockin: ticker could not open database: {e}");
                    return;
                }
            };
            loop {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                let running = db.pomo_state().running;
                if running {
                    let _ = db.pomo_tick();
                }
            }
        })
        .ok();
}

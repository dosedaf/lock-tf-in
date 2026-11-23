# LOCK IN

Native desktop rewrite of the old OBS todo / pomodoro / tracker dashboard.
Same 1998 Valve-site design, no web view, no Python server, no GTK/WebKit shell.

- Rust + `eframe`/`egui` (OpenGL), one binary.
- SQLite via `rusqlite`, same `todo.db` schema as the Python version.
- Render on demand: 0% idle CPU; timer wakes four times a second only while running.
- Timer keeps running while the window is hidden; system notifications on phase end.

## Build

```sh
cargo build --release -p lockin
./target/release/lockin
```

Database resolution, first match wins:

1. `--db PATH`
2. `$LOCKIN_DB`
3. `~/projects/obs-todo/todo.db` if present
4. `$XDG_DATA_HOME/lockin/todo.db`

## Shortcuts

| key | action |
|---|---|
| `Ctrl+1..5` | tasks / tracker / timer / summary / log |
| `Space` | start / pause timer |
| `Ctrl+W` | hide to background |
| `Ctrl+Q` | quit |
| right click task | state menu |

Closing the window hides it; launching the binary again raises the running instance.
No autostart is installed.

## Layout

```
crates/core   domain + SQLite + pomodoro state machine + PNG cards + tracker/summary
crates/app    native egui UI, theme, views
assets        ceramic.gif, arrow.gif, Liberation Sans, icon
packaging     desktop entry for installing later
```

The original Python/web version (`server.py`, the GTK/WebKit shell, the systemd
unit) is no longer in the tree. It lives in git history at `fc21d23` if you ever
need to look back.

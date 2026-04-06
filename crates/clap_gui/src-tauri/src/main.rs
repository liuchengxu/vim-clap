#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod state;

use state::AppState;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Shared visibility state so quit_app can "hide" the window.
pub struct WindowVisibility {
    visible: AtomicBool,
    /// Stored logical size to restore after un-hiding.
    width: AtomicU32,
    height: AtomicU32,
}

/// Detect the git repository root from the current directory.
fn git_repo_root() -> Option<PathBuf> {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8(o.stdout)
                .ok()
                .map(|s| PathBuf::from(s.trim()))
        })
}

/// Hide the window. On Linux, shrinks to 1x1 to keep the event loop and
/// global key grabs alive. On other platforms, uses native hide.
fn hide_window(win: &tauri::WebviewWindow, vis: &WindowVisibility) {
    if vis.visible.load(Ordering::Relaxed) {
        // Save current physical size before hiding.
        if let Ok(size) = win.outer_size() {
            vis.width.store(size.width, Ordering::Relaxed);
            vis.height.store(size.height, Ordering::Relaxed);
        }

        if cfg!(target_os = "linux") {
            let _ = win.set_always_on_top(false);
            let _ = win.set_size(tauri::PhysicalSize::new(1u32, 1u32));
        } else {
            let _ = win.hide();
        }

        vis.visible.store(false, Ordering::Relaxed);
    }
}

/// Show the window centered on screen.
fn show_window(win: &tauri::WebviewWindow, vis: &WindowVisibility) {
    use tauri::Emitter;

    let w = vis.width.load(Ordering::Relaxed);
    let h = vis.height.load(Ordering::Relaxed);

    if cfg!(target_os = "linux") {
        let _ = win.set_size(tauri::PhysicalSize::new(w, h));
        let _ = win.set_always_on_top(true);
    } else {
        let _ = win.show();
    }

    // Center on screen.
    if let Some(monitor) = win.current_monitor().ok().flatten() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let screen_w = screen.width as f64 / scale;
        let screen_h = screen.height as f64 / scale;
        let lw = w as f64 / scale;
        let lh = h as f64 / scale;
        let x = ((screen_w - lw) / 2.0).round();
        let y = ((screen_h - lh) / 2.0).round();
        let _ = win.set_position(tauri::LogicalPosition::new(x, y));
    }

    vis.visible.store(true, Ordering::Relaxed);

    // On Linux, window managers block focus stealing. Use xdotool to force it.
    let win2 = win.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let _ = win2.set_focus();

        if cfg!(target_os = "linux") {
            // Use wmctrl to force-activate by PID — more reliable than xdotool on many WMs.
            let pid = std::process::id().to_string();
            let _ = Command::new("wmctrl")
                .args(["-x", "-a", "clap-gui"])
                .status();
            // Fallback: xdotool by PID.
            let _ = Command::new("xdotool")
                .args(["search", "--pid", &pid, "--onlyvisible", "windowactivate"])
                .status();
        }

        let _ = win2.emit("window-shown", ());
    });
}

fn main() {
    let config = config::load_config();

    let args: Vec<String> = std::env::args().collect();
    let cwd = args
        .get(1)
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| git_repo_root().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }));

    let hotkey = config.window.hotkey.clone();
    let app_state = AppState::new(cwd, config);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::search::start_search,
            commands::search::refresh_file_cache,
            commands::search::get_cwd,
            commands::search::quit_app,
            commands::search::force_quit,
            commands::preview::preview_file,
            commands::action::open_in_editor,
        ])
        .setup(move |app| {
            use tauri::Manager;
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

            if let Some(window) = app.get_webview_window("main") {
                // Resize window proportionally to the monitor size.
                if let Some(monitor) = window.current_monitor().ok().flatten() {
                    let screen = monitor.size();
                    let scale = monitor.scale_factor();
                    let screen_w = screen.width as f64 / scale;
                    let screen_h = screen.height as f64 / scale;

                    let w = (screen_w * 0.6).round();
                    let h = (screen_h * 0.55).round();

                    let _ = window.set_size(tauri::LogicalSize::new(w, h));
                    let x = ((screen_w - w) / 2.0).round();
                    let y = ((screen_h - h) / 2.0).round();
                    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                }

                let _ = window.set_always_on_top(true);
                let _ = window.set_visible_on_all_workspaces(true);
                let _ = window.set_focus();

                // Initialize visibility state (stored as physical pixels).
                let win_size = window.outer_size().unwrap_or(tauri::PhysicalSize::new(800, 500));

                let win_vis = Arc::new(WindowVisibility {
                    visible: AtomicBool::new(true),
                    width: AtomicU32::new(win_size.width),
                    height: AtomicU32::new(win_size.height),
                });
                app.manage(win_vis.clone());

                // Register global hotkey to toggle window visibility.
                if let Err(e) = hotkey.parse::<Shortcut>() {
                    eprintln!("Failed to parse hotkey '{hotkey}': {e}");
                }
                if let Ok(shortcut) = hotkey.parse::<Shortcut>() {
                    let win = window.clone();
                    let wv = win_vis.clone();
                    let _ = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                        if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                            return;
                        }
                        if wv.visible.load(Ordering::Relaxed) {
                            hide_window(&win, &wv);
                        } else {
                            show_window(&win, &wv);
                        }
                    });
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running clap-gui");
}

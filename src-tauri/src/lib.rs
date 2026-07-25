//! Nod — a menu-bar answer to "will the Mac fall asleep if I walk away now?"
//!
//! The icon carries the answer at all times; clicking it opens the list of what
//! is holding the machine awake, with a cross to close each one. The work of
//! deciding lives in `sleep` (who is holding) and `watch` (what that means);
//! this file is the wiring: tray, popover, polling, updates.

mod debug_log;
mod mac_window;
mod private;
mod quit;
mod sleep;
mod watch;

use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

use std::sync::Mutex;
use watch::Mood;

/// How often the icon re-checks. Assertions come and go on the scale of minutes,
/// and `pmset` is a process spawn — a tighter loop would buy nothing and burn
/// battery on a tool whose whole point is battery.
const POLL_SECS: u64 = 30;

/// The popover is authored at this width; its height is whatever the content
/// turns out to be — the page measures itself and calls `fit_popover`.
const POPOVER_WIDTH: f64 = 292.0;

/// A height arriving from the page is a number from a webview, so it is bounded
/// before it becomes a window size.
const POPOVER_MIN_HEIGHT: f64 = 60.0;
const POPOVER_MAX_HEIGHT: f64 = 640.0;

/// Gap between the menu bar and the popover, matching the system menus.
const POPOVER_GAP: f64 = 6.0;

// Three states, and each of them can also be carrying an update badge. The
// colour says whether the Mac will sleep; the green dot says a new version is
// out. They are different questions, so they cannot share the green.
const ICON_CALM: &[u8] = include_bytes!("../icons/tray-calm.png");
const ICON_BLOCKED: &[u8] = include_bytes!("../icons/tray-blocked.png");
const ICON_CHARGING: &[u8] = include_bytes!("../icons/tray-charging.png");
const ICON_CALM_UPDATE: &[u8] = include_bytes!("../icons/tray-calm-update.png");
const ICON_BLOCKED_UPDATE: &[u8] = include_bytes!("../icons/tray-blocked-update.png");
const ICON_CHARGING_UPDATE: &[u8] = include_bytes!("../icons/tray-charging-update.png");

fn tray_icon(mood: Mood, update_waiting: bool) -> &'static [u8] {
    match (mood, update_waiting) {
        (Mood::Calm, false) => ICON_CALM,
        (Mood::Blocked, false) => ICON_BLOCKED,
        (Mood::Charging, false) => ICON_CHARGING,
        (Mood::Calm, true) => ICON_CALM_UPDATE,
        (Mood::Blocked, true) => ICON_BLOCKED_UPDATE,
        (Mood::Charging, true) => ICON_CHARGING_UPDATE,
    }
}

/// What the popover and the icon are both drawn from.
#[derive(serde::Serialize, Clone)]
struct Verdict {
    mood: Mood,
    on_battery: bool,
    /// Minutes the Mac is configured to wait before sleeping on battery.
    sleep_after: u32,
    holders: Vec<sleep::Holder>,
}

/// Runtime state the poll loop carries between ticks.
struct Watcher {
    /// `None` until the first tick: launching while already on battery is not
    /// the same event as pulling the cord, and must not fire a notification.
    was_on_battery: Mutex<Option<bool>>,
    update_waiting: Mutex<bool>,
}

fn look(min_age: u64) -> Verdict {
    let on_battery = sleep::on_battery();
    let holders = watch::settled(&sleep::holders(), min_age);
    Verdict {
        mood: watch::mood(on_battery, holders.len()),
        on_battery,
        sleep_after: sleep::battery_timers().sleep,
        holders,
    }
}

fn paint(app: &AppHandle, mood: Mood) {
    let waiting = app
        .try_state::<Watcher>()
        .and_then(|w| w.update_waiting.lock().ok().map(|g| *g))
        .unwrap_or(false);
    if let Some(tray) = app.tray_by_id("main") {
        if let Ok(icon) = tauri::image::Image::from_bytes(tray_icon(mood, waiting)) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}

#[tauri::command]
fn get_verdict(app: AppHandle) -> Verdict {
    let verdict = look(watch::SETTLED_SECS);
    paint(&app, verdict.mood);
    verdict
}

#[tauri::command]
fn close_holder(app: AppHandle, pid: u32) -> Result<(), String> {
    // The page names a pid, and this is the only place in the app that can end
    // a process — so the pid is checked against what is actually holding the
    // Mac awake right now, rather than trusted. The list is the permission:
    // whatever the popover is showing, it may close, and nothing else. Without
    // this, anything that got a foothold in the webview could quit any process
    // the user owns, and the app names in that view come from other people's
    // software.
    if !sleep::holders().iter().any(|h| h.pid == pid) {
        debug_log::log(&format!("quit: refused pid {} — not a current holder", pid));
        return Err("that app is no longer holding the Mac awake".into());
    }
    quit::ask_to_quit(pid)?;
    // The app needs a moment to put itself away before pmset stops listing it.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        let verdict = look(watch::SETTLED_SECS);
        paint(&handle, verdict.mood);
        let _ = handle.emit("verdict-changed", verdict);
    });
    Ok(())
}

#[tauri::command]
fn fit_popover(app: AppHandle, height: f64) {
    let Some(window) = app.get_webview_window(POPOVER) else { return };
    let height = height.clamp(POPOVER_MIN_HEIGHT, POPOVER_MAX_HEIGHT);
    let _ = window.set_size(tauri::LogicalSize::new(POPOVER_WIDTH, height));
}

#[tauri::command]
fn js_log(msg: String) {
    debug_log::log(&format!("ui: {}", msg));
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<serde_json::Value, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            debug_log::log(&format!("update: v{} available", update.version));
            announce_update(&app, &update.version);
            Ok(serde_json::json!({ "available": true, "version": update.version }))
        }
        Ok(None) => Ok(serde_json::json!({ "available": false })),
        Err(e) => {
            debug_log::log(&format!("update: check failed: {}", e));
            Err(e.to_string())
        }
    }
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            debug_log::log(&format!("update: downloading v{}", update.version));
            update
                .download_and_install(|_, _| {}, || debug_log::log("update: downloaded, restarting"))
                .await
                .map_err(|e| e.to_string())?;
            app.restart();
        }
        Ok(None) => Err("No updates available".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Badge the icon and turn the menu's first item into the install action.
/// Called from both the manual check and the background poll.
fn announce_update(app: &AppHandle, version: &str) {
    if let Some(item) = app.try_state::<MenuItem<Wry>>() {
        let _ = item.set_text(format!("Update to v{}", version));
    }
    if let Some(w) = app.try_state::<Watcher>() {
        if let Ok(mut waiting) = w.update_waiting.lock() {
            *waiting = true;
        }
    }
    paint(app, look(watch::SETTLED_SECS).mood);
    let _ = app.emit("update-available", version);
}

/// One menu item, two jobs: check when nothing is pending, install once a
/// version has been found. Two items would leave a dead "Check" sitting next to
/// a live "Update".
async fn on_update_clicked(app: AppHandle) {
    match check_for_update(app.clone()).await {
        Ok(v) if v["available"] == serde_json::Value::Bool(true) => {
            if let Err(e) = install_update(app).await {
                debug_log::log(&format!("update: install failed: {}", e));
            }
        }
        Ok(_) => debug_log::log("update: nothing to install"),
        Err(e) => debug_log::log(&format!("update: check failed: {}", e)),
    }
}

/// Put the popover under the menu-bar icon and show it.
///
/// The tray click hands us where the icon is, which beats guessing: on a Mac
/// with several screens, or with other menu-bar apps coming and going, a
/// window parked at a remembered position ends up somewhere else entirely.
fn show_popover(app: &AppHandle, icon: tauri::Rect) {
    let Some(window) = app.get_webview_window(POPOVER) else {
        debug_log::log("popover: window is gone");
        return;
    };

    if let (tauri::Position::Physical(pos), tauri::Size::Physical(size)) = (icon.position, icon.size)
    {
        let scale = window.scale_factor().unwrap_or(1.0);
        let centre = pos.x as f64 + size.width as f64 / 2.0;
        let x = centre / scale - POPOVER_WIDTH / 2.0;
        let y = (pos.y as f64 + size.height as f64) / scale + POPOVER_GAP;
        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    }

    mac_window::show_popover(app);
}

fn toggle_popover(app: &AppHandle, icon: tauri::Rect) {
    if mac_window::popover_visible(app) {
        mac_window::hide_popover(app);
        return;
    }
    let verdict = look(watch::SETTLED_SECS);
    paint(app, verdict.mood);
    let _ = app.emit("verdict-changed", verdict);
    show_popover(app, icon);
}

pub fn run() {
    debug_log::init();

    tauri::Builder::default()
        .plugin(mac_window::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .manage(Watcher {
            was_on_battery: Mutex::new(None),
            update_waiting: Mutex::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            // Only what the popover actually calls. The update commands are
            // driven from the tray menu, in Rust, so exposing them to the page
            // would widen the surface for nothing.
            get_verdict,
            close_holder,
            js_log,
            fit_popover
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Menu-bar utility: no Dock icon, no Cmd-Tab entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            build_tray(app)?;

            // Before anything shows it: the popover only behaves — appears on
            // the Space in front of the user, stays up while another app is
            // active — once it is an NSPanel rather than a window.
            if let Some(window) = handle.get_webview_window(POPOVER) {
                if let Err(e) = mac_window::setup_panel(&window) {
                    debug_log::log(&format!("panel: setup failed: {}", e));
                }
            }
            mac_window::dismiss_on_outside_click(handle.clone());

            let poll = handle.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tick(&poll);
                    tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
                }
            });

            // The app sits in the tray for weeks, so a release that ships while
            // it runs has to badge the icon on its own.
            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                loop {
                    if let Ok(updater) = update_handle.updater() {
                        match updater.check().await {
                            Ok(Some(update)) => {
                                announce_update(&update_handle, &update.version);
                                break; // badge is on — nothing left to poll for
                            }
                            Ok(None) => {}
                            Err(e) => debug_log::log(&format!("update: poll failed: {}", e)),
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                }
            });

            debug_log::log("setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            // Clicking away puts the popover down, the way a menu closes. It is
            // hidden, never destroyed — the tray icon opens the same window
            // every time. On macOS the click is caught by the NSEvent monitor in
            // `mac_window` instead: the panel is never the active app, so losing
            // focus there means nothing and hiding on it made the popover blink
            // out the instant it appeared.
            if window.label() == POPOVER {
                match event {
                    #[cfg(not(target_os = "macos"))]
                    tauri::WindowEvent::Focused(false) => {
                        let _ = window.hide();
                    }
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    _ => {}
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Nod");
}

/// One turn of the loop: repaint the icon, and speak up if the charger just
/// came out while something is holding the Mac awake.
fn tick(app: &AppHandle) {
    let on_battery = sleep::on_battery();
    let was = app
        .try_state::<Watcher>()
        .and_then(|w| w.was_on_battery.lock().ok().map(|g| *g))
        .unwrap_or(None);
    let unplugged = watch::just_unplugged(was, on_battery);

    let all = sleep::holders();
    let min_age = if unplugged { watch::UNPLUG_SECS } else { watch::SETTLED_SECS };
    let holders = watch::settled(&all, min_age);
    let mood = watch::mood(on_battery, holders.len());
    paint(app, mood);

    if let Some(w) = app.try_state::<Watcher>() {
        if let Ok(mut last) = w.was_on_battery.lock() {
            *last = Some(on_battery);
        }
    }

    if unplugged {
        if let Some(worst) = watch::worst(&holders) {
            let minutes = worst.held / 60;
            let extra = holders.len() - 1;
            let mut body = format!("{} has held it for {} min", worst.app, minutes);
            if extra > 0 {
                body.push_str(&format!(" (and {} more)", extra));
            }
            debug_log::log(&format!("unplugged with {} holders: {}", holders.len(), body));
            let _ = app
                .notification()
                .builder()
                .title("It won't fall asleep")
                .body(body)
                .show();
        } else {
            debug_log::log("unplugged, nothing holding it");
        }
    }

    let _ = app.emit(
        "verdict-changed",
        Verdict {
            mood,
            on_battery,
            sleep_after: sleep::battery_timers().sleep,
            holders,
        },
    );
}

/// The popover. Everything else in the config would be a window the tray opens,
/// and every one of those must be hidden on close rather than destroyed.
const POPOVER: &str = "main";

/// Left click opens the popover — the list *is* the app, and hiding it behind a
/// menu would put a click between the question and its answer. The menu stays
/// on the right button, where the update, the version and Quit live, same as in
/// Ribbit, Quill and Iago.
fn build_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let update = MenuItem::with_id(app, "update", "Check for updates", true, None::<&str>)?;
    let version = MenuItem::with_id(
        app,
        "version",
        format!("Nod v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Nod", true, None::<&str>)?;

    let menu = MenuBuilder::new(app)
        .item(&update)
        .separator()
        .item(&version)
        .item(&quit_item)
        .build()?;

    // announce_update() rewrites this item's text when a release lands.
    app.manage(update.clone());

    let icon = tauri::image::Image::from_bytes(ICON_CHARGING)?;
    TrayIconBuilder::with_id("main")
        .tooltip("Nod — will the Mac fall asleep?")
        .icon(icon)
        .icon_as_template(false)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, rect, .. } = event {
                if button == tauri::tray::MouseButton::Left {
                    toggle_popover(tray.app_handle(), rect);
                }
            }
        })
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "update" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    on_update_clicked(app).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mood_has_its_own_icon() {
        let moods = [Mood::Calm, Mood::Blocked, Mood::Charging];
        for (i, a) in moods.iter().enumerate() {
            for b in moods.iter().skip(i + 1) {
                assert_ne!(
                    tray_icon(*a, false),
                    tray_icon(*b, false),
                    "{:?} and {:?} share an icon — the colour is the whole signal",
                    a,
                    b
                );
            }
        }
    }

    #[test]
    fn the_update_icon_carries_the_green_badge() {
        // The badge has to be visible in every state, not only the calm one:
        // an update that lands while something holds the Mac awake would
        // otherwise never announce itself.
        for mood in [Mood::Calm, Mood::Blocked, Mood::Charging] {
            assert_ne!(
                tray_icon(mood, false),
                tray_icon(mood, true),
                "{:?} shows the same icon with and without a pending update",
                mood
            );
        }
    }

    #[test]
    fn the_popover_is_the_only_window() {
        // A second window would need hiding on close, or its tray item opens
        // nothing after the first time. This test is the reminder.
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let labels: Vec<_> = conf["app"]["windows"]
            .as_array()
            .expect("config has no windows")
            .iter()
            .map(|w| w["label"].as_str().expect("window without a label"))
            .collect();
        assert_eq!(labels, [POPOVER]);
    }
}

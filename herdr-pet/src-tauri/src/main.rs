#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use tauri::{State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

mod api;
mod monitor;
mod state;

use state::PetState;

const WINDOW_WIDTH: f64 = 320.0;
const WINDOW_HEIGHT: f64 = 176.0;

struct SharedState(Arc<Mutex<PetState>>);

#[tauri::command]
fn get_state(state: State<'_, SharedState>) -> PetState {
    state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

#[tauri::command]
fn start_drag(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(target_os = "macos")]
fn configure_macos_window(window: &WebviewWindow) -> tauri::Result<()> {
    use objc2_app_kit::{NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior};

    let pointer = window.ns_window()?;
    let window = unsafe { &*pointer.cast::<NSWindow>() };
    let behavior = window.collectionBehavior()
        | NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary;
    window.setCanHide(false);
    window.setHidesOnDeactivate(false);
    window.setCollectionBehavior(behavior);
    window.setLevel(NSScreenSaverWindowLevel);
    window.setHasShadow(false);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn configure_macos_window(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

fn main() {
    let shared = Arc::new(Mutex::new(PetState::default()));
    let monitor_state = Arc::clone(&shared);

    tauri::Builder::default()
        .manage(SharedState(shared))
        .invoke_handler(tauri::generate_handler![get_state, start_drag, quit])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                app.set_dock_visibility(false);
            }

            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Herdr Pet")
                    .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
                    .center()
                    .visible(false)
                    .transparent(true)
                    .decorations(false)
                    .resizable(false)
                    .maximizable(false)
                    .minimizable(false)
                    .always_on_top(true)
                    .visible_on_all_workspaces(true)
                    .skip_taskbar(true)
                    .shadow(false)
                    .focused(false)
                    .build()?;

            configure_macos_window(&window)?;
            window.show()?;
            monitor::start(app.handle().clone(), Arc::clone(&monitor_state));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run herdr-pet");
}

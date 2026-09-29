#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::menu::{CheckMenuItemBuilder, ContextMenu, MenuBuilder, SubmenuBuilder};
use tauri::{
    Emitter, LogicalPosition, Manager, PhysicalPosition, Position, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

mod api;
mod monitor;
mod platform;
mod state;

use state::PetState;

const WINDOW_WIDTH: f64 = 320.0;
const WINDOW_HEIGHT: f64 = 620.0;
const DEFAULT_PET_LEFT: f64 = 12.0;
// 宠物停靠窗口底部,预留 12px 边距。数值必须和 renderer/index.html 里
// canvas 的 height 属性一致,JS 启动时会以 canvas 属性为准上报。
const DEFAULT_PET_TOP: f64 = WINDOW_HEIGHT - 45.0 - 12.0;
const MOVE_SETTLE_DELAY: Duration = Duration::from_millis(140);

#[derive(Debug, Clone, Copy, PartialEq)]
struct PetSize {
    width: f64,
    height: f64,
}

pub(crate) struct SharedState {
    pub(crate) snapshot: PetState,
    pub(crate) socket_path: PathBuf,
}

struct ManagedState {
    shared: Arc<Mutex<SharedState>>,
    pet_size: Arc<Mutex<Option<PetSize>>>,
}
struct MonitorControl(Sender<monitor::Control>);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PetLayout {
    pet_left: f64,
    pet_top: f64,
}

impl Default for PetLayout {
    fn default() -> Self {
        Self {
            pet_left: DEFAULT_PET_LEFT,
            pet_top: DEFAULT_PET_TOP,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedSettings {
    window_x: Option<i32>,
    window_y: Option<i32>,
    #[serde(default)]
    layout: PetLayout,
}

struct SettingsStore {
    path: PathBuf,
    value: Mutex<PersistedSettings>,
}

impl SettingsStore {
    fn load(path: PathBuf) -> Self {
        let value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default();
        Self {
            path,
            value: Mutex::new(value),
        }
    }

    fn snapshot(&self) -> PersistedSettings {
        self.value
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn save(&self, value: PersistedSettings) {
        {
            let mut current = self
                .value
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *current = value.clone();
        }
        let Some(parent) = self.path.parent() else {
            return;
        };
        if let Err(error) = std::fs::create_dir_all(parent) {
            eprintln!("herdr-pet: failed to create settings directory: {error}");
            return;
        }
        let content = match serde_json::to_vec_pretty(&value) {
            Ok(content) => content,
            Err(error) => {
                eprintln!("herdr-pet: failed to serialize settings: {error}");
                return;
            }
        };
        if let Err(error) = std::fs::write(&self.path, content) {
            eprintln!(
                "herdr-pet: failed to write settings to {}: {error}",
                self.path.display()
            );
        }
    }
}

#[tauri::command]
fn get_state(state: State<'_, ManagedState>) -> PetState {
    state
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot
        .clone()
}

#[tauri::command]
fn get_layout(settings: State<'_, Arc<SettingsStore>>) -> PetLayout {
    settings.snapshot().layout
}

#[tauri::command]
fn set_pet_size(state: State<'_, ManagedState>, width: f64, height: f64) {
    if !(width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0) {
        return;
    }
    let mut size = state
        .pet_size
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *size = Some(PetSize { width, height });
}

#[tauri::command]
fn start_drag(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    std::thread::spawn(move || {
        // macOS start_dragging returns once the native drag begins, so a
        // detached watcher reports the actual button release to the renderer.
        while platform::primary_mouse_button_down() {
            std::thread::sleep(Duration::from_millis(30));
        }
        let _ = window.emit("pet-drag-ended", ());
    });
    Ok(())
}

#[tauri::command]
fn set_prompt_active(window: WebviewWindow, active: bool) -> Result<(), String> {
    platform::set_prompt_active(&window, active).map_err(|error| error.to_string())
}

#[tauri::command]
fn focus_agent(
    pane_id: String,
    state: State<'_, ManagedState>,
    control: State<'_, MonitorControl>,
    window: WebviewWindow,
) -> Result<(), String> {
    let path = state
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .socket_path
        .clone();
    control
        .0
        .send(monitor::Control::SelectAgent(pane_id.clone()))
        .map_err(|error| error.to_string())?;
    run_focus_request(window, path, pane_id);
    Ok(())
}

#[tauri::command]
async fn send_prompt(
    pane_id: String,
    text: String,
    session: String,
    state: State<'_, ManagedState>,
) -> Result<(), String> {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err("提示内容不能为空".to_string());
    }
    let path = {
        let shared = state
            .shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if shared.snapshot.session != session {
            return Err("会话已切换，请返回列表重新选择 agent".to_string());
        }
        shared.socket_path.clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        api::request(
            &path,
            "agent.prompt",
            serde_json::json!({"target": pane_id, "text": text}),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn show_settings_menu(
    window: tauri::Window,
    state: State<'_, ManagedState>,
    english: bool,
    menu_x: f64,
    menu_y: f64,
) -> Result<(), String> {
    let snapshot = state
        .shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .snapshot
        .clone();
    let app = window.app_handle();
    let classic = CheckMenuItemBuilder::with_id(
        "skin-classic",
        if english {
            "Classic pixel"
        } else {
            "经典像素"
        },
    )
    .checked(true)
    .build(app)
    .map_err(|error| error.to_string())?;
    let skin = SubmenuBuilder::new(app, if english { "Skin" } else { "皮肤" })
        .item(&classic)
        .build()
        .map_err(|error| error.to_string())?;

    let mut sessions = SubmenuBuilder::new(app, if english { "Session" } else { "会话" });
    for session in &snapshot.sessions {
        let item = CheckMenuItemBuilder::with_id(
            format!("select-session:{}", session.name),
            &session.name,
        )
        .checked(session.active)
        .build(app)
        .map_err(|error| error.to_string())?;
        sessions = sessions.item(&item);
    }
    let sessions = sessions.build().map_err(|error| error.to_string())?;
    let chinese = CheckMenuItemBuilder::with_id("language-zh", "中文")
        .checked(!english)
        .build(app)
        .map_err(|error| error.to_string())?;
    let english_item = CheckMenuItemBuilder::with_id("language-en", "English")
        .checked(english)
        .build(app)
        .map_err(|error| error.to_string())?;
    let language = SubmenuBuilder::new(app, if english { "Language" } else { "语言" })
        .item(&chinese)
        .item(&english_item)
        .build()
        .map_err(|error| error.to_string())?;

    let mut menu = MenuBuilder::new(app);
    if snapshot.sessions.len() > 1 {
        menu = menu.item(&sessions).separator();
    }
    let menu = menu
        .item(&language)
        .item(&skin)
        .separator()
        .text("quit", if english { "Quit" } else { "退出" })
        .build()
        .map_err(|error| error.to_string())?;
    menu.popup_at(
        window,
        Position::Logical(LogicalPosition::new(menu_x, menu_y)),
    )
    .map_err(|error| error.to_string())
}

fn run_focus_request(window: WebviewWindow, path: PathBuf, pane_id: String) {
    std::thread::spawn(move || {
        if let Err(error) =
            api::request(&path, "agent.focus", serde_json::json!({"target": pane_id}))
        {
            let _ = window.emit("pet-error", error.to_string());
            return;
        }
        if let Err(error) = api::activate_client(&path) {
            let _ = window.emit(
                "pet-error",
                format!("已切换 agent，但无法显示 Herdr 窗口：{error}"),
            );
        }
    });
}

fn handle_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    if let Some(language) = id.strip_prefix("language-") {
        let _ = app.emit("pet-language", language);
        return;
    }
    if id == "quit" {
        app.exit(0);
        return;
    }
    if let Some(session) = id.strip_prefix("select-session:") {
        if let Some(control) = app.try_state::<MonitorControl>() {
            let _ = control
                .0
                .send(monitor::Control::SelectSession(session.to_string()));
        }
    }
}

fn start_position_worker(
    window: WebviewWindow,
    settings: Arc<SettingsStore>,
    pet_size: Arc<Mutex<Option<PetSize>>>,
) {
    let (sender, receiver) = mpsc::sync_channel(1);
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Moved(_)) {
            let _ = sender.try_send(());
        }
    });
    std::thread::spawn(move || loop {
        if receiver.recv().is_err() {
            break;
        }
        loop {
            match receiver.recv_timeout(MOVE_SETTLE_DELAY) {
                Ok(()) => continue,
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        while platform::primary_mouse_button_down() {
            std::thread::sleep(Duration::from_millis(30));
        }
        let size = *pet_size
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        normalize_window(&window, &settings, size);
    });
}

fn normalize_window(window: &WebviewWindow, settings: &SettingsStore, pet_size: Option<PetSize>) {
    let Some(pet_size) = pet_size else {
        // The renderer has not reported the canvas size yet; keep the current
        // window position until it does so the pet is never clamped against
        // dimensions that do not match the actual sprite.
        return;
    };
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let Ok(scale) = window.scale_factor() else {
        return;
    };
    let previous = settings.snapshot();
    let pet_global_x = f64::from(position.x) + previous.layout.pet_left * scale;
    let pet_global_y = f64::from(position.y) + previous.layout.pet_top * scale;
    let monitor = window.available_monitors().ok().and_then(|monitors| {
        monitors.into_iter().find(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            pet_global_x >= f64::from(origin.x)
                && pet_global_x < f64::from(origin.x) + f64::from(size.width)
                && pet_global_y >= f64::from(origin.y)
                && pet_global_y < f64::from(origin.y) + f64::from(size.height)
        })
    });
    let monitor = match monitor {
        Some(monitor) => monitor,
        None => match window.current_monitor() {
            Ok(Some(monitor)) => monitor,
            _ => match window.primary_monitor() {
                Ok(Some(monitor)) => monitor,
                _ => return,
            },
        },
    };
    let origin = monitor.position();
    let monitor_size = monitor.size();
    let wanted_x = (pet_global_x - DEFAULT_PET_LEFT * scale).round() as i64;
    let wanted_y = (pet_global_y - DEFAULT_PET_TOP * scale).round() as i64;
    let minimum_x = i64::from(origin.x);
    let minimum_y = i64::from(origin.y);
    let maximum_x = minimum_x + i64::from(monitor_size.width.saturating_sub(size.width));
    let maximum_y = minimum_y + i64::from(monitor_size.height.saturating_sub(size.height));
    let x = wanted_x.clamp(minimum_x, maximum_x) as i32;
    let y = wanted_y.clamp(minimum_y, maximum_y) as i32;
    let layout = PetLayout {
        pet_left: ((pet_global_x - f64::from(x)) / scale)
            .clamp(0.0, (WINDOW_WIDTH - pet_size.width).max(0.0)),
        pet_top: ((pet_global_y - f64::from(y)) / scale)
            .clamp(0.0, (WINDOW_HEIGHT - pet_size.height).max(0.0)),
    };
    if position.x != x || position.y != y {
        let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    }
    settings.save(PersistedSettings {
        window_x: Some(x),
        window_y: Some(y),
        layout,
    });
    let _ = window.emit("pet-layout", layout);
}

fn main() {
    let shared = Arc::new(Mutex::new(SharedState {
        snapshot: PetState::default(),
        socket_path: api::socket_path(),
    }));
    let monitor_state = Arc::clone(&shared);
    let pet_size = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .manage(ManagedState {
            shared,
            pet_size: Arc::clone(&pet_size),
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            get_layout,
            set_pet_size,
            start_drag,
            set_prompt_active,
            focus_agent,
            send_prompt,
            show_settings_menu
        ])
        .on_menu_event(handle_menu_event)
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                app.set_dock_visibility(false);
            }

            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = Arc::new(SettingsStore::load(settings_path));
            let saved = settings.snapshot();
            app.manage(Arc::clone(&settings));

            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Herdr Pet")
                    .accept_first_mouse(true)
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

            if let (Some(x), Some(y)) = (saved.window_x, saved.window_y) {
                window.set_position(Position::Physical(PhysicalPosition::new(x, y)))?;
            }
            platform::configure_window(&window)?;
            start_position_worker(window.clone(), settings, Arc::clone(&pet_size));
            window.show()?;
            let control = monitor::start(app.handle().clone(), Arc::clone(&monitor_state));
            app.manage(MonitorControl(control));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run herdr-pet");
}

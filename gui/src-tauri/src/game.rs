//! Game detection, launch state and the play button.

use crate::prefs::{self, LaunchBehavior};
use crate::{lifecycle, notify, NotificationLevel};
use base64::Engine;
use sc_launch::{launch, profiles, steam};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Checking,
    Ready,
    Launching,
    Running,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    revision: u64,
    phase: Phase,
    primary_action: Option<&'static str>,
    busy: bool,
    blocking_reason: Option<String>,
    game_directory: Option<String>,
    proton: Option<String>,
}

#[derive(Default)]
struct Inner {
    revision: u64,
    phase: Option<Phase>,
    reason: Option<String>,
    setup: Option<launch::Setup>,
}

#[derive(Default)]
pub struct GameState(Mutex<Inner>);

fn snapshot_of(inner: &Inner) -> Snapshot {
    let phase = inner.phase.unwrap_or(Phase::Checking);
    #[cfg(unix)]
    let proton = inner.setup.as_ref().map(|s| s.proton.dir.display().to_string());
    #[cfg(not(unix))]
    let proton = None;
    Snapshot {
        revision: inner.revision,
        phase,
        primary_action: match phase {
            Phase::Ready => Some("launch"),
            Phase::Failed => Some("retry"),
            _ => None,
        },
        busy: matches!(phase, Phase::Checking | Phase::Launching),
        blocking_reason: inner.reason.clone(),
        game_directory: inner.setup.as_ref().map(|s| s.game_dir.display().to_string()),
        proton,
    }
}

fn update(app: &AppHandle, f: impl FnOnce(&mut Inner)) {
    let snapshot = {
        let state = app.state::<GameState>();
        let mut inner = state.0.lock().unwrap();
        f(&mut inner);
        inner.revision += 1;
        snapshot_of(&inner)
    };
    if let Err(e) = app.emit_to("main", "launcher-state-changed", snapshot) {
        eprintln!("could not publish launcher state: {e}");
    }
}

fn phase(app: &AppHandle) -> Phase {
    app.state::<GameState>().0.lock().unwrap().phase.unwrap_or(Phase::Checking)
}

/// Where sc-inject.exe and sc_agent.dll are: bundled resources, next to this
/// program, or (for development builds) the repo's dist folder.
fn tools_dir(app: &AppHandle) -> Option<PathBuf> {
    let has_tools = |d: &Path| d.join(launch::INJECT_EXE).is_file() && d.join(launch::AGENT_DLL).is_file();
    let mut candidates = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("tools"));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidates.push(dir.clone());
        // target/<profile>/ -> <repo>/dist/<platform>/
        let platform = if cfg!(windows) { "windows" } else { "linux" };
        if let Some(repo) = dir.ancestors().find(|a| a.join("dist").join(platform).is_dir()) {
            candidates.push(repo.join("dist").join(platform));
        }
    }
    candidates.into_iter().find(|d| has_tools(d))
}

fn options(app: &AppHandle) -> launch::Options {
    let p = prefs::get(app);
    let path = |s: &str| (!s.trim().is_empty()).then(|| PathBuf::from(s.trim()));
    launch::Options {
        game_dir: path(&p.game_directory),
        tools_dir: tools_dir(app),
        proton: path(&p.proton_path),
    }
}

/// Re-detects the game and Proton in the background.
pub fn refresh(app: &AppHandle) {
    if matches!(phase(app), Phase::Launching | Phase::Running) {
        return;
    }
    update(app, |s| {
        s.phase = Some(Phase::Checking);
        s.reason = None;
    });
    let app = app.clone();
    std::thread::spawn(move || {
        let result = launch::resolve(&options(&app));
        update(&app, |s| match result {
            Ok(setup) => {
                s.setup = Some(setup);
                s.phase = Some(Phase::Ready);
            }
            Err(e) => {
                s.setup = None;
                s.phase = Some(Phase::Failed);
                s.reason = Some(e);
            }
        });
    });
}

fn launch_log() -> Option<PathBuf> {
    profiles::data_dir().map(|d| d.join("game-launch.log"))
}

fn start_game(app: &AppHandle) -> Result<(), String> {
    let setup = app
        .state::<GameState>()
        .0
        .lock()
        .unwrap()
        .setup
        .clone()
        .ok_or("the game setup has not been detected yet")?;
    let selected = prefs::get(app).selected_profile;
    if selected.is_empty() || !profiles::valid_name(&selected) {
        return Err("Choose a server profile first.".into());
    }
    let dir = crate::profiles::profiles_dir()?.join(&selected);
    profiles::load(&dir)?;
    let mut cmd = launch::command(&setup, &dir, &[])?;

    // Proton and the injector are chatty; keep their output for troubleshooting.
    let (out, err) = match launch_log().and_then(|p| {
        p.parent().map(std::fs::create_dir_all);
        std::fs::File::create(p).ok()
    }) {
        Some(f) => (
            f.try_clone().map(Stdio::from).unwrap_or_else(|_| Stdio::null()),
            Stdio::from(f),
        ),
        None => (Stdio::null(), Stdio::null()),
    };
    cmd.stdin(Stdio::null()).stdout(out).stderr(err);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| format!("could not start the game: {e}"))?;
    update(app, |s| s.phase = Some(Phase::Running));

    match prefs::get(app).launch_behavior {
        LaunchBehavior::Tray => {
            if let Err(e) = lifecycle::hide_to_tray(app) {
                eprintln!("could not hide the launcher: {e}");
            }
        }
        LaunchBehavior::KeepOpen => {}
        LaunchBehavior::Exit => {
            // The game keeps running on its own.
            app.exit(0);
            return Ok(());
        }
    }

    let app = app.clone();
    std::thread::spawn(move || {
        let status = child.wait();
        update(&app, |s| s.phase = Some(Phase::Ready));
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => eprintln!("game exited with {s}"),
            Err(e) => eprintln!("could not wait for the game: {e}"),
        }
        if prefs::get(&app).launch_behavior == LaunchBehavior::Tray {
            lifecycle::restore(&app);
        }
    });
    Ok(())
}

#[tauri::command]
pub fn get_launcher_snapshot(app: AppHandle) -> Snapshot {
    snapshot_of(&app.state::<GameState>().0.lock().unwrap())
}

#[tauri::command]
pub fn initialize_launcher(app: AppHandle) {
    refresh(&app);
}

#[tauri::command]
pub fn perform_launcher_action(app: AppHandle, action: String) -> Result<(), String> {
    match (action.as_str(), phase(&app)) {
        ("retry", Phase::Failed | Phase::Ready) => {
            refresh(&app);
            Ok(())
        }
        ("launch", Phase::Ready) => {
            update(&app, |s| s.phase = Some(Phase::Launching));
            start_game(&app).inspect_err(|e| {
                update(&app, |s| s.phase = Some(Phase::Ready));
                notify(&app, e.clone(), NotificationLevel::Error);
            })
        }
        _ => Err("This action is not available right now.".into()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompatTools {
    supported: bool,
    proton: Vec<String>,
}

#[tauri::command]
pub fn detect_compat_tools() -> CompatTools {
    #[cfg(unix)]
    {
        let roots = steam::steam_roots();
        let libraries = steam::libraries(&roots);
        CompatTools {
            supported: true,
            proton: sc_launch::compat::proton_candidates(&roots, &libraries)
                .into_iter()
                .map(|p| p.display().to_string())
                .collect(),
        }
    }
    #[cfg(not(unix))]
    CompatTools { supported: false, proton: Vec::new() }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    hero: Option<String>,
    logo: Option<String>,
}

/// A file from Steam's library artwork cache for the game, in either the
/// current (`<appid>/<hash>/<name>`) or older (`<appid>_<name>`) layout.
fn steam_art(name: &str) -> Option<PathBuf> {
    for root in steam::steam_roots() {
        let cache = root.join("appcache").join("librarycache");
        let flat = cache.join(format!("{}_{name}", steam::GAME_APPID));
        if flat.is_file() {
            return Some(flat);
        }
        let Ok(entries) = std::fs::read_dir(cache.join(steam::GAME_APPID)) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.join(name).is_file() {
                return Some(p.join(name));
            }
            if p.file_name().is_some_and(|f| f == name) && p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn data_url(path: &Path) -> Option<String> {
    let mime = match path.extension()?.to_str()? {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return None,
    };
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 8 << 20 {
        return None;
    }
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Star Conflict artwork from the local Steam install (nothing is bundled).
#[tauri::command]
pub async fn get_artwork() -> Artwork {
    Artwork {
        hero: steam_art("library_hero.jpg").and_then(|p| data_url(&p)),
        logo: steam_art("logo.png").and_then(|p| data_url(&p)),
    }
}

#[tauri::command]
pub fn view_game_install(app: AppHandle) -> Result<(), String> {
    let dir = app
        .state::<GameState>()
        .0
        .lock()
        .unwrap()
        .setup
        .as_ref()
        .map(|s| s.game_dir.clone())
        .ok_or("The game folder has not been found yet.")?;
    crate::open_path(&app, &dir)
}

#[tauri::command]
pub fn view_launch_log(app: AppHandle) -> Result<(), String> {
    let log = launch_log().filter(|p| p.is_file()).ok_or("The game has not been started from the launcher yet.")?;
    crate::open_path(&app, &log)
}

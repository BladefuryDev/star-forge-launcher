use sc_profile::{normalize, Profile, AGENT_LOG, PROFILE_ENV};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;

pub struct State {
    /// Game install root (the folder holding win64\, data\, cloud\).
    pub game_root: String,
    pub profile_dir: String,
    pub trace: std::sync::atomic::AtomicBool,
    log: Mutex<Option<File>>,
    /// Lower-cased relative path -> replacement path (None = not in profile).
    pub redirects: Mutex<HashMap<String, Option<Vec<u16>>>>,
}

static STATE: OnceLock<State> = OnceLock::new();

pub fn state() -> Option<&'static State> {
    STATE.get()
}

pub fn log(msg: &str) {
    if let Some(s) = STATE.get() {
        if let Ok(mut guard) = s.log.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(f, "{msg}");
            }
        }
    }
}

fn exe_path() -> String {
    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetModuleFileNameW(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) };
    String::from_utf16_lossy(&buf[..n as usize])
}

/// Sets the agent up. Returns false when the game must not start.
pub fn init() -> bool {
    let Ok(profile_dir) = std::env::var(PROFILE_ENV) else {
        return true; // Not launched by sc-launch: do nothing.
    };
    let profile_dir = normalize(&profile_dir);
    let log_file = File::create(format!("{profile_dir}\\{AGENT_LOG}")).ok();

    // game.exe lives in <root>\win64\.
    let exe = normalize(&exe_path());
    let game_root = exe
        .rsplitn(3, '\\')
        .nth(2)
        .unwrap_or_default()
        .to_string();

    let _ = STATE.set(State {
        game_root: game_root.clone(),
        profile_dir: profile_dir.clone(),
        trace: std::sync::atomic::AtomicBool::new(false),
        log: Mutex::new(log_file),
        redirects: Mutex::new(HashMap::new()),
    });
    log(&format!("sc-agent {} loaded into {exe}", env!("CARGO_PKG_VERSION")));
    log(&format!("game root: {game_root}"));
    log(&format!("profile:   {profile_dir}"));

    let profile = match Profile::load(std::path::Path::new(&profile_dir)) {
        Ok(p) => p,
        Err(e) => {
            log(&format!("profile error, agent disabled: {e}"));
            return true;
        }
    };
    if profile.trace_files {
        if let Some(s) = STATE.get() {
            s.trace.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        log("tracing file access");
    }
    if !profile.name.is_empty() {
        log(&format!("profile name: {}", profile.name));
    }

    if !crate::patches::apply_all(&profile_dir, &profile) {
        return false;
    }
    if profile.own_game_data {
        crate::folders::install(&sc_profile::state_local_appdata(&profile_dir));
    }

    if game_root.is_empty() {
        log("could not work out the game root; file redirects disabled");
        return true;
    }
    crate::hooks::install();
    true
}

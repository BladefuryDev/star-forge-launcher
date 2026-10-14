//! Server profiles: list, add, edit and delete from the launcher.

use sc_launch::profiles;
use sc_profile::{Profile, FILES_DIR, PROFILE_FILE};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

pub fn profiles_dir() -> Result<PathBuf, String> {
    profiles::default_profiles_dir().ok_or_else(|| "cannot work out the profiles folder".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    id: String,
    name: String,
    server: Option<String>,
    local_server: bool,
    patches: usize,
    error: Option<String>,
}

#[tauri::command]
pub fn list_profiles() -> Result<Vec<ProfileInfo>, String> {
    Ok(profiles::list(&profiles_dir()?)
        .into_iter()
        .map(|e| match e.profile {
            Ok(p) => ProfileInfo {
                name: if p.name.trim().is_empty() { e.name.clone() } else { p.name.clone() },
                id: e.name,
                server: p.server.clone(),
                local_server: p.local_server,
                patches: p.patches.len(),
                error: None,
            },
            Err(err) => ProfileInfo {
                name: e.name.clone(),
                id: e.name,
                server: None,
                local_server: true,
                patches: 0,
                error: Some(err),
            },
        })
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    id: Option<String>,
    name: String,
    server: String,
    local_server: bool,
}

/// Folder name for a new profile: lower-case letters, digits and dashes.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').chars().take(40).collect::<String>();
    if out.is_empty() { "profile".into() } else { out }
}

/// Removes an `address.cfg` this launcher generated, so clearing the server
/// field really clears it. A hand-written file is left alone.
fn remove_generated_address(dir: &std::path::Path) {
    let cfg = dir.join(FILES_DIR).join("data").join("address.cfg");
    if let Ok(text) = std::fs::read_to_string(&cfg) {
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.len() == 1 && lines[0].starts_with("set net_masterServerAddress ") {
            let _ = std::fs::remove_file(cfg);
        }
    }
}

#[tauri::command]
pub fn save_profile(app: AppHandle, profile: ProfileInput) -> Result<String, String> {
    let root = profiles_dir()?;
    let name = profile.name.trim();
    if name.is_empty() {
        return Err("Give the profile a name.".into());
    }
    if name.chars().count() > 64 {
        return Err("The profile name is too long (64 characters at most).".into());
    }
    let server = profile.server.trim();

    let id = match profile.id {
        Some(id) => {
            if !profiles::valid_name(&id) || !root.join(&id).join(PROFILE_FILE).is_file() {
                return Err(format!("profile {id:?} does not exist"));
            }
            id
        }
        None => {
            let base = slug(name);
            let mut id = base.clone();
            let mut n = 2;
            while root.join(&id).exists() {
                id = format!("{base}-{n}");
                n += 1;
            }
            id
        }
    };
    let dir = root.join(&id);
    let file = dir.join(PROFILE_FILE);

    // Edit the existing file so patches, args and comments are kept.
    let mut doc: toml_edit::DocumentMut = match std::fs::read_to_string(&file) {
        Ok(text) => text.parse().map_err(|e| format!("{}: {e}", file.display()))?,
        Err(_) => toml_edit::DocumentMut::new(),
    };
    doc["name"] = toml_edit::value(name);
    if server.is_empty() {
        doc.remove("server");
    } else {
        doc["server"] = toml_edit::value(server);
    }
    if profile.local_server {
        doc.remove("local_server");
    } else {
        doc["local_server"] = toml_edit::value(false);
    }
    let text = doc.to_string();
    let parsed = Profile::from_toml(&text).map_err(|e| e.to_string())?;
    parsed
        .address_cfg()
        .map_err(|_| "The server address can only use letters, digits, '.', '-', '_' and ':' (for example STARFURY.XYZ:3801).".to_string())?;

    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".{PROFILE_FILE}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &file).map_err(|e| format!("{}: {e}", file.display()))?;
    if server.is_empty() {
        remove_generated_address(&dir);
    }
    crate::prefs::modify(&app, |p| {
        if p.selected_profile.is_empty() {
            p.selected_profile = id.clone();
        }
    })?;
    Ok(id)
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    let root = profiles_dir()?;
    let dir = root.join(&id);
    if !profiles::valid_name(&id) || !dir.join(PROFILE_FILE).is_file() {
        return Err(format!("profile {id:?} does not exist"));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    crate::prefs::modify(&app, |p| {
        if p.selected_profile == id {
            p.selected_profile.clear();
        }
    })?;
    Ok(())
}

#[tauri::command]
pub fn open_profile_folder(app: AppHandle, id: Option<String>) -> Result<(), String> {
    let root = profiles_dir()?;
    let dir = match id {
        Some(id) if profiles::valid_name(&id) && root.join(&id).is_dir() => root.join(id),
        Some(id) => return Err(format!("profile {id:?} does not exist")),
        None => {
            std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
            root
        }
    };
    crate::open_path(&app, &dir)
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs() {
        assert_eq!(slug("Starfury community server"), "starfury-community-server");
        assert_eq!(slug("  Local (dev) "), "local-dev");
        assert_eq!(slug("!!!"), "profile");
    }
}

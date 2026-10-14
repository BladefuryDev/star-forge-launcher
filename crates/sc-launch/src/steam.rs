//! Finding Steam, its library folders and installed apps.

use std::path::{Path, PathBuf};

/// Star Conflict's Steam app id.
pub const GAME_APPID: &str = "212070";

/// Quoted tokens of a text VDF file, with `\\` and `\"` unescaped.
fn vdf_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut tok = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        tok.push(n);
                    }
                }
                '"' => break,
                _ => tok.push(c),
            }
        }
        out.push(tok);
    }
    out
}

/// Every value following a quoted `key` (case-insensitive). Good enough for the
/// flat lookups needed here; not a full VDF parser.
pub fn vdf_values(text: &str, key: &str) -> Vec<String> {
    let toks = vdf_tokens(text);
    toks.windows(2)
        .filter(|w| w[0].eq_ignore_ascii_case(key))
        .map(|w| w[1].clone())
        .collect()
}

/// Likely Steam install roots on this OS, existing ones only.
pub fn steam_roots() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    {
        if let Some(p) = windows_registry_steam_path() {
            out.push(p);
        }
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(pf) = std::env::var_os(var) {
                out.push(PathBuf::from(pf).join("Steam"));
            }
        }
    }
    #[cfg(unix)]
    {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            out.push(home.join(".local/share/Steam"));
            out.push(home.join(".steam/steam"));
            out.push(home.join(".steam/root"));
            out.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
            #[cfg(target_os = "macos")]
            out.push(home.join("Library/Application Support/Steam"));
        }
    }
    let mut seen = Vec::new();
    out.into_iter()
        .filter(|p| p.join("steamapps").is_dir())
        .filter(|p| {
            let key = std::fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        })
        .collect()
}

#[cfg(windows)]
fn windows_registry_steam_path() -> Option<PathBuf> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let key: Vec<u16> = "Software\\Valve\\Steam\0".encode_utf16().collect();
    let value: Vec<u16> = "SteamPath\0".encode_utf16().collect();
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if rc != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
}

/// All library folders known to the given Steam roots (roots included).
pub fn libraries(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for root in roots {
        let mut add = |p: PathBuf| {
            if p.join("steamapps").is_dir() && !out.contains(&p) {
                out.push(p);
            }
        };
        add(root.clone());
        if let Ok(text) = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for p in vdf_values(&text, "path") {
                add(PathBuf::from(p));
            }
        }
    }
    out
}

/// Library folder and install directory of an app, from its appmanifest.
pub fn find_app(libraries: &[PathBuf], appid: &str) -> Option<(PathBuf, PathBuf)> {
    libraries.iter().find_map(|lib| {
        let text = std::fs::read_to_string(lib.join(format!("steamapps/appmanifest_{appid}.acf"))).ok()?;
        let dir = vdf_values(&text, "installdir").into_iter().next()?;
        let path = lib.join("steamapps/common").join(dir);
        path.is_dir().then(|| (lib.clone(), path))
    })
}

/// True if `dir` looks like a Star Conflict install.
pub fn is_game_dir(dir: &Path) -> bool {
    dir.join("win64/game.exe").is_file() && dir.join("data").is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_library_paths_with_escapes() {
        let text = r#""libraryfolders" { "0" { "path" "/home/u/.local/share/Steam" "apps" { "212070" "1" } }
            "1" { "path" "D:\\Games\\Steam" } }"#;
        assert_eq!(
            vdf_values(text, "path"),
            vec!["/home/u/.local/share/Steam".to_string(), r"D:\Games\Steam".to_string()]
        );
    }

    #[test]
    fn reads_installdir_and_tool_appid() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"212070\"\n\t\"installdir\"\t\t\"star conflict\"\n}";
        assert_eq!(vdf_values(acf, "installdir"), vec!["star conflict".to_string()]);
        let manifest = "\"manifest\"\n{\n  \"require_tool_appid\" \"4183110\"\n}";
        assert_eq!(vdf_values(manifest, "require_tool_appid"), vec!["4183110".to_string()]);
    }
}

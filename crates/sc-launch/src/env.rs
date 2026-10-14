//! Machine-specific overrides, read from `STAR_FORGE_*` environment variables.
//! Explicit settings (command-line flags, launcher preferences) win over these,
//! and these win over auto-detection. See `.env.example` at the repo root.

use std::path::PathBuf;

/// Star Conflict install folder (holding win64\ and data\).
pub const GAME_DIR: &str = "STAR_FORGE_GAME_DIR";
/// Folder holding sc-inject.exe and sc_agent.dll.
pub const TOOLS_DIR: &str = "STAR_FORGE_TOOLS_DIR";
/// Proton build folder (Linux).
pub const PROTON: &str = "STAR_FORGE_PROTON";
/// Launcher data folder (preferences, profiles, launch log).
pub const DATA_DIR: &str = "STAR_FORGE_DATA_DIR";

/// A path variable, ignoring unset and empty values.
pub fn path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Development builds read `<repo>/.env` so overrides apply however the binary
/// is started (terminal, desktop entry). Variables already set are kept.
/// Release builds never read a .env file. Call before starting any threads.
pub fn load_dotenv() {
    if cfg!(debug_assertions) {
        let file = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env");
        if let Ok(text) = std::fs::read_to_string(file) {
            for (key, value) in parse_dotenv(&text) {
                if std::env::var_os(&key).is_none() {
                    std::env::set_var(key, value);
                }
            }
        }
    }
}

/// `KEY=value` lines; blank lines and `#` comments are skipped, an optional
/// `export ` prefix and matching surrounding quotes are dropped.
fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (key, value) = l.strip_prefix("export ").unwrap_or(l).split_once('=')?;
            let value = value.trim();
            let value = [('"', '"'), ('\'', '\'')]
                .iter()
                .find_map(|&(a, b)| value.strip_prefix(a)?.strip_suffix(b))
                .unwrap_or(value);
            Some((key.trim().to_string(), value.to_string()))
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dotenv_lines() {
        let text = "# comment\n\nSTAR_FORGE_GAME_DIR=\"/games/star conflict\"\nexport STAR_FORGE_PROTON='/p'\nEMPTY=\nnot a line\n";
        assert_eq!(
            parse_dotenv(text),
            vec![
                ("STAR_FORGE_GAME_DIR".into(), "/games/star conflict".into()),
                ("STAR_FORGE_PROTON".into(), "/p".into()),
                ("EMPTY".into(), String::new()),
            ]
        );
    }
}

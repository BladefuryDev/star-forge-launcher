//! Profile format and path rules shared by the launcher and the agent.
//!
//! Everything here is plain string logic so it can be unit tested on Linux;
//! paths are Windows-style strings because both consumers run under Windows/Wine.

use serde::Deserialize;
use std::fmt;

/// Name of the profile description file inside a profile folder.
pub const PROFILE_FILE: &str = "profile.toml";
/// Folder inside a profile that mirrors the game root (`files\data\address.cfg`, ...).
pub const FILES_DIR: &str = "files";
/// Environment variable sc-inject uses to tell the agent which profile is active.
pub const PROFILE_ENV: &str = "SC_PROFILE";
/// Log file the agent writes inside the profile folder.
pub const AGENT_LOG: &str = "agent.log";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Master server `host:port`. When set, sc-inject writes
    /// `files\data\address.cfg` so the login screen uses this server.
    pub server: Option<String>,
    /// Start the bundled local server (MongoDB + Hub) with the game, as Steam's
    /// default `-launchLocalServer` does. Turn off for profiles that only play
    /// on a remote server.
    #[serde(default = "yes")]
    pub local_server: bool,
    /// Extra arguments passed to game.exe.
    #[serde(default)]
    pub args: Vec<String>,
    /// Log every game path the agent sees (debugging only; can be large).
    #[serde(default)]
    pub trace_files: bool,
    /// In-memory byte patches applied by the agent before the game runs.
    #[serde(default, rename = "patch")]
    pub patches: Vec<PatchDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchDef {
    #[serde(default)]
    pub description: String,
    /// Module the offset is relative to. Defaults to the game executable.
    #[serde(default = "default_module")]
    pub module: String,
    /// Offset from the module base (RVA), e.g. `"0x1A2B3C"`.
    pub offset: String,
    /// Bytes expected at the offset before patching, hex (spaces allowed).
    pub original: String,
    /// Bytes to write, hex. Must be the same length as `original`.
    pub patched: String,
}

fn yes() -> bool {
    true
}

fn default_module() -> String {
    "game.exe".to_string()
}

/// A validated patch ready to apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub description: String,
    pub module: String,
    pub offset: usize,
    pub original: Vec<u8>,
    pub patched: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl Profile {
    pub fn from_toml(text: &str) -> Result<Self, Error> {
        toml::from_str(text).map_err(|e| Error(format!("{PROFILE_FILE}: {e}")))
    }

    /// Full game.exe argument list.
    pub fn args(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.local_server {
            out.push("-launchLocalServer".to_string());
        }
        out.extend(self.args.iter().cloned());
        out
    }

    /// Validates every patch definition.
    pub fn patches(&self) -> Result<Vec<Patch>, Error> {
        self.patches.iter().map(PatchDef::validate).collect()
    }

    /// Contents of the generated `address.cfg`, if a server is set.
    pub fn address_cfg(&self) -> Result<Option<String>, Error> {
        let Some(server) = &self.server else { return Ok(None) };
        let ok = !server.is_empty()
            && server
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'));
        if !ok {
            return Err(Error(format!("server {server:?}: expected host:port")));
        }
        Ok(Some(format!("set net_masterServerAddress \"{server}\"\r\n")))
    }
}

impl PatchDef {
    pub fn validate(&self) -> Result<Patch, Error> {
        let what = if self.description.is_empty() { &self.offset } else { &self.description };
        let offset = parse_offset(&self.offset).map_err(|e| Error(format!("patch {what}: {e}")))?;
        let original = parse_hex(&self.original).map_err(|e| Error(format!("patch {what}: original: {e}")))?;
        let patched = parse_hex(&self.patched).map_err(|e| Error(format!("patch {what}: patched: {e}")))?;
        if original.is_empty() || original.len() != patched.len() {
            return Err(Error(format!(
                "patch {what}: original and patched must be the same non-zero length ({} vs {})",
                original.len(),
                patched.len()
            )));
        }
        if !is_plain_file_name(&self.module) {
            return Err(Error(format!("patch {what}: module must be a file name, got {:?}", self.module)));
        }
        Ok(Patch {
            description: self.description.clone(),
            module: self.module.clone(),
            offset,
            original,
            patched,
        })
    }
}

fn is_plain_file_name(s: &str) -> bool {
    !s.is_empty() && !s.contains(['\\', '/', ':']) && s != "." && s != ".."
}

pub fn parse_offset(s: &str) -> Result<usize, String> {
    let t = s.trim();
    let (digits, radix) = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(h) => (h, 16),
        None => (t, 10),
    };
    usize::from_str_radix(&digits.replace('_', ""), radix).map_err(|_| format!("bad offset {s:?}"))
}

pub fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let digits: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.len() % 2 != 0 {
        return Err(format!("odd number of hex digits in {s:?}"));
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).map_err(|_| format!("bad hex in {s:?}")))
        .collect()
}

/// Turns an argument into a Windows path. Under Wine a Unix absolute path
/// (`/home/...`) is reachable through the `Z:` drive.
pub fn windows_path_from_arg(arg: &str) -> String {
    if arg.starts_with('/') {
        format!("Z:{}", arg.replace('/', "\\"))
    } else {
        arg.to_string()
    }
}

/// Canonical form used for comparisons: no `\\?\` prefix, backslashes,
/// no repeated or trailing separators.
pub fn normalize(path: &str) -> String {
    let p = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\??\"))
        .unwrap_or(path);
    let mut out = String::with_capacity(p.len());
    for c in p.chars() {
        let c = if c == '/' { '\\' } else { c };
        if c == '\\' && out.ends_with('\\') {
            continue;
        }
        out.push(c);
    }
    while out.len() > 3 && out.ends_with('\\') {
        out.pop();
    }
    out
}

/// If `full` (an absolute path) lies inside `root`, returns the path relative to
/// it. Rejects anything that could escape a profile folder or address a stream.
pub fn relative_to(root: &str, full: &str) -> Option<String> {
    let root = normalize(root);
    let full = normalize(full);
    if full.len() <= root.len() + 1 {
        return None;
    }
    let (head, rest) = full.split_at(root.len());
    if !head.eq_ignore_ascii_case(&root) || !rest.starts_with('\\') {
        return None;
    }
    let rel = &rest[1..];
    let safe = rel
        .split('\\')
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':'));
    safe.then(|| rel.to_string())
}

/// Path inside the profile that would replace `rel`.
pub fn profile_file(profile_dir: &str, rel: &str) -> String {
    format!("{}\\{}\\{}", normalize(profile_dir), FILES_DIR, rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r"Z:\home\user\.local\share\Steam\steamapps\common\star conflict";

    #[test]
    fn relative_inside_root() {
        let full = format!(r"{ROOT}\data\address.cfg");
        assert_eq!(relative_to(ROOT, &full).as_deref(), Some(r"data\address.cfg"));
    }

    #[test]
    fn relative_is_case_insensitive_and_slash_tolerant() {
        let full = r"z:/HOME/user/.local/share/Steam/steamapps/common/Star Conflict//data/ui/x.lua";
        assert_eq!(relative_to(ROOT, full).as_deref(), Some(r"data\ui\x.lua"));
    }

    #[test]
    fn relative_handles_verbatim_prefix() {
        let full = format!(r"\\?\{ROOT}\data\a.cfg");
        assert_eq!(relative_to(ROOT, &full).as_deref(), Some(r"data\a.cfg"));
    }

    #[test]
    fn relative_rejects_outside_and_sibling() {
        assert_eq!(relative_to(ROOT, r"Z:\home\user\other\data\a.cfg"), None);
        assert_eq!(relative_to(ROOT, &format!("{ROOT}2\\data\\a.cfg")), None);
        assert_eq!(relative_to(ROOT, ROOT), None);
        assert_eq!(relative_to(ROOT, &format!("{ROOT}\\")), None);
    }

    #[test]
    fn relative_rejects_traversal_and_streams() {
        assert_eq!(relative_to(ROOT, &format!(r"{ROOT}\data\..\..\x")), None);
        assert_eq!(relative_to(ROOT, &format!(r"{ROOT}\data\.\x")), None);
        assert_eq!(relative_to(ROOT, &format!(r"{ROOT}\data\x.cfg:evil")), None);
    }

    #[test]
    fn unix_args_map_to_z_drive() {
        assert_eq!(windows_path_from_arg("/home/user/p"), r"Z:\home\user\p");
        assert_eq!(windows_path_from_arg(r"C:\p"), r"C:\p");
    }

    #[test]
    fn profile_file_joins() {
        assert_eq!(profile_file(r"Z:\p\", r"data\a.cfg"), r"Z:\p\files\data\a.cfg");
    }

    #[test]
    fn hex_and_offsets() {
        assert_eq!(parse_hex("90 90\tEB").unwrap(), vec![0x90, 0x90, 0xEB]);
        assert!(parse_hex("909").is_err());
        assert!(parse_hex("zz").is_err());
        assert_eq!(parse_offset("0x1A_2B").unwrap(), 0x1A2B);
        assert_eq!(parse_offset("42").unwrap(), 42);
    }

    #[test]
    fn profile_parses_and_validates() {
        let p = Profile::from_toml(
            r#"
            name = "Starfury"
            server = "starfury.xyz:3801"

            [[patch]]
            description = "nop a check"
            offset = "0x1000"
            original = "74 05"
            patched = "90 90"
            "#,
        )
        .unwrap();
        assert_eq!(p.args(), vec!["-launchLocalServer"]);
        let client_only = Profile::from_toml("local_server = false\nargs = [\"-x\"]").unwrap();
        assert_eq!(client_only.args(), vec!["-x"]);
        let patches = p.patches().unwrap();
        assert_eq!(patches[0].module, "game.exe");
        assert_eq!(patches[0].offset, 0x1000);
        assert_eq!(
            p.address_cfg().unwrap().unwrap(),
            "set net_masterServerAddress \"starfury.xyz:3801\"\r\n"
        );
    }

    #[test]
    fn profile_rejects_bad_input() {
        assert!(Profile::from_toml("bogus = 1").is_err());
        let p = Profile::from_toml(r#"server = "evil\"; quit""#).unwrap();
        assert!(p.address_cfg().is_err());
        let p = Profile::from_toml(
            "[[patch]]\noffset = \"0x10\"\noriginal = \"90\"\npatched = \"90 90\"",
        )
        .unwrap();
        assert!(p.patches().is_err());
        let p = Profile::from_toml(
            "[[patch]]\nmodule = \"..\\\\x.dll\"\noffset = \"0x10\"\noriginal = \"90\"\npatched = \"91\"",
        )
        .unwrap();
        assert!(p.patches().is_err());
    }
}

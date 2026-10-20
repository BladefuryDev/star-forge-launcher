//! Profile format and path rules shared by the launcher and the agent.
//!
//! A profile is a folder holding a server pack (`pack.json`, `resources\`,
//! `files\`) as published by a server operator, plus the player's own
//! `local.json`, which pack updates never touch. [`Profile`] is the two merged.
//!
//! Everything here is plain string logic so it can be unit tested on Linux;
//! paths are Windows-style strings because both consumers run under Windows/Wine.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

/// The pack manifest inside a profile folder.
pub const PACK_FILE: &str = "pack.json";
/// The player's settings inside a profile folder; never part of a pack.
pub const LOCAL_FILE: &str = "local.json";
/// Folder inside a profile that mirrors the game root (`files\data\address.cfg`, ...).
pub const FILES_DIR: &str = "files";
/// Folder inside a pack holding launcher artwork.
pub const RESOURCES_DIR: &str = "resources";
/// Environment variable sc-inject uses to tell the agent which profile is active.
pub const PROFILE_ENV: &str = "SC_PROFILE";
/// Log file the agent writes inside the profile folder.
pub const AGENT_LOG: &str = "agent.log";
/// Newest pack format this build understands.
pub const PACK_FORMAT: u32 = 1;

/// `pack.json`. See `schemas/pack.schema.json` for the documented format.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub format: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    #[serde(default, skip_serializing_if = "Links::is_empty")]
    pub links: Links,
    #[serde(default, skip_serializing_if = "Requires::is_empty")]
    pub requires: Requires,
    #[serde(default)]
    pub server: Server,
    #[serde(default, skip_serializing_if = "Game::is_empty")]
    pub game: Game,
    #[serde(default, skip_serializing_if = "Branding::is_empty")]
    pub branding: Branding,
    /// Experimental: how client patches are applied is likely to change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<PatchDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<Update>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<Download>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Links {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub news: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Links {
    fn is_empty(&self) -> bool {
        self.entries().next().is_none()
    }

    /// `(key, url)` for every link that is set, in display order.
    pub fn entries(&self) -> impl Iterator<Item = (&'static str, &str)> {
        [("website", &self.website), ("discord", &self.discord), ("news", &self.news), ("source", &self.source)]
            .into_iter()
            .filter_map(|(k, v)| Some((k, v.as_deref()?)))
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// Minimum launcher version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher: Option<String>,
    /// SHA-256 of the `win64\game.exe` builds the pack was made for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game: Vec<String>,
}

impl Requires {
    fn is_empty(&self) -> bool {
        self.launcher.is_none() && self.game.is_empty()
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    /// Master server `host:port`, written to `files\data\address.cfg`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Also start the game's bundled local server (MongoDB + Hub).
    #[serde(default)]
    pub local: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

impl Game {
    fn is_empty(&self) -> bool {
        self.args.is_empty()
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Branding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
}

impl Branding {
    fn is_empty(&self) -> bool {
        self.logo.is_none() && self.icon.is_none() && self.background.is_none() && self.accent.is_none()
    }
}

/// Where the launcher looks for newer versions of the pack.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    /// URL of the published `pack.json` (the copy with `download` filled in).
    pub url: String,
}

/// The pack's zip, in the published `pack.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Download {
    pub url: String,
    /// SHA-256 of the zip, hex.
    pub sha256: String,
}

/// `local.json`: the player's changes on top of the pack.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Local {
    /// Display name instead of the pack's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Server address instead of the pack's; an empty string means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_server: Option<bool>,
    /// Arguments added after the pack's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Log every game path the agent sees (debugging only; can be large).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub trace_files: bool,
}

/// A profile ready to launch: the pack with the player's settings applied.
#[derive(Debug, Default)]
pub struct Profile {
    pub name: String,
    pub server: Option<String>,
    pub local_server: bool,
    pub args: Vec<String>,
    pub trace_files: bool,
    pub patches: Vec<PatchDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchDef {
    #[serde(default, skip_serializing_if = "String::is_empty")]
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

impl Pack {
    pub fn from_json(text: &str) -> Result<Self, Error> {
        serde_json::from_str(text).map_err(|e| Error(format!("{PACK_FILE}: {e}")))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("pack serializes") + "\n"
    }

    /// Checks everything that doesn't need the pack's files.
    pub fn validate(&self) -> Result<(), Error> {
        let err = |m: String| Err(Error(format!("{PACK_FILE}: {m}")));
        if self.format == 0 || self.format > PACK_FORMAT {
            return err(format!(
                "format {} is not supported by this launcher (it reads format {PACK_FORMAT}); update the launcher",
                self.format
            ));
        }
        if !valid_pack_id(&self.id) {
            return err(format!("id {:?}: use lower-case letters, digits, '.' and '-' (e.g. org.example.server)", self.id));
        }
        if self.name.trim().is_empty() {
            return err("name is empty".into());
        }
        if parse_version(&self.version).is_none() {
            return err(format!("version {:?}: expected x.y.z", self.version));
        }
        if let Some(a) = &self.server.address {
            check_address(a)?;
        }
        for (key, path) in [("logo", &self.branding.logo), ("icon", &self.branding.icon), ("background", &self.branding.background)] {
            if let Some(p) = path {
                if resource_rel(p).is_none() {
                    return err(format!("branding.{key} {p:?}: must be a file inside resources/"));
                }
            }
        }
        if let Some(c) = &self.branding.accent {
            let hex = c.strip_prefix('#').filter(|h| h.len() == 6 && h.chars().all(|ch| ch.is_ascii_hexdigit()));
            if hex.is_none() {
                return err(format!("branding.accent {c:?}: expected a colour like #e39a29"));
            }
        }
        for h in &self.requires.game {
            if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
                return err(format!("requires.game {h:?}: expected a SHA-256 in hex"));
            }
        }
        if let Some(d) = &self.download {
            if d.sha256.len() != 64 || !d.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return err("download.sha256: expected a SHA-256 in hex".into());
            }
        }
        self.patches.iter().try_for_each(|p| p.validate().map(drop))
    }
}

/// Pack ids are a reversed domain or similar: `[a-z0-9]` runs joined by `.` or `-`.
pub fn valid_pack_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.split(['.', '-']).all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
}

/// `x.y.z` with an optional `-pre`/`+build` suffix (ignored when comparing).
pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|n| n.parse::<u64>().ok());
    let out = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(out)
}

/// The path inside `resources/` a branding entry points at, with `/` separators.
pub fn resource_rel(path: &str) -> Option<&str> {
    let rel = path.strip_prefix(RESOURCES_DIR)?.strip_prefix('/')?;
    let ok = !rel.is_empty()
        && !rel.contains(['\\', ':'])
        && rel.split('/').all(|part| !part.is_empty() && part != "." && part != "..");
    ok.then_some(rel)
}

fn check_address(server: &str) -> Result<(), Error> {
    let ok = !server.is_empty()
        && server.len() <= 255
        && server
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'));
    if ok {
        Ok(())
    } else {
        Err(Error(format!("server {server:?}: expected host:port")))
    }
}

impl Local {
    pub fn from_json(text: &str) -> Result<Self, Error> {
        serde_json::from_str(text).map_err(|e| Error(format!("{LOCAL_FILE}: {e}")))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("settings serialize") + "\n"
    }
}

impl Profile {
    pub fn new(pack: &Pack, local: &Local) -> Self {
        let server = match &local.server {
            Some(s) if s.is_empty() => None,
            Some(s) => Some(s.clone()),
            None => pack.server.address.clone(),
        };
        let mut args = pack.game.args.clone();
        args.extend(local.args.iter().cloned());
        Profile {
            name: local.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| pack.name.clone()),
            server,
            local_server: local.local_server.unwrap_or(pack.server.local),
            args,
            trace_files: local.trace_files,
            patches: pack.patches.clone(),
        }
    }

    /// Reads `pack.json` and the optional `local.json` from a profile folder.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let pack_path = dir.join(PACK_FILE);
        let text = std::fs::read_to_string(&pack_path).map_err(|e| Error(format!("{}: {e}", pack_path.display())))?;
        let pack = Pack::from_json(&text)?;
        pack.validate()?;
        let local = match std::fs::read_to_string(dir.join(LOCAL_FILE)) {
            Ok(text) => Local::from_json(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Local::default(),
            Err(e) => return Err(Error(format!("{LOCAL_FILE}: {e}"))),
        };
        let profile = Profile::new(&pack, &local);
        profile.address_cfg()?;
        Ok(profile)
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
        check_address(server)?;
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


    fn pack(json: &str) -> Pack {
        Pack::from_json(json).unwrap()
    }

    const MINIMAL: &str = r#"{"format":1,"id":"xyz.starfury.main","name":"Starfury","version":"1.0.0"}"#;

    #[test]
    fn pack_parses_and_merges_local_settings() {
        let p = pack(
            r#"{
              "$schema": "x", "format": 1, "id": "xyz.starfury.main", "name": "Starfury", "version": "1.2.0",
              "server": { "address": "starfury.xyz:3801" },
              "game": { "args": ["-a"] },
              "patches": [{ "description": "nop a check", "offset": "0x1000", "original": "74 05", "patched": "90 90" }]
            }"#,
        );
        p.validate().unwrap();
        let plain = Profile::new(&p, &Local::default());
        assert_eq!(plain.name, "Starfury");
        assert_eq!(plain.args(), vec!["-a"]);
        assert_eq!(plain.address_cfg().unwrap().unwrap(), "set net_masterServerAddress \"starfury.xyz:3801\"\r\n");
        let patches = plain.patches().unwrap();
        assert_eq!(patches[0].module, "game.exe");
        assert_eq!(patches[0].offset, 0x1000);

        let local = Local::from_json(r#"{"name":"Mine","server":"","local_server":true,"args":["-b"],"trace_files":true}"#).unwrap();
        let mine = Profile::new(&p, &local);
        assert_eq!(mine.name, "Mine");
        assert_eq!(mine.server, None);
        assert_eq!(mine.args(), vec!["-launchLocalServer", "-a", "-b"]);
        assert!(mine.trace_files);
    }

    #[test]
    fn pack_rejects_bad_input() {
        assert!(Pack::from_json(r#"{"format":1,"id":"a","name":"a","version":"1.0.0","bogus":1}"#).is_err());
        let bad = |edit: &str| {
            let mut v: serde_json::Value = serde_json::from_str(MINIMAL).unwrap();
            let e: serde_json::Value = serde_json::from_str(edit).unwrap();
            for (k, x) in e.as_object().unwrap() {
                v[k] = x.clone();
            }
            Pack::from_json(&v.to_string()).unwrap().validate().is_err()
        };
        assert!(!bad("{}"));
        assert!(bad(r#"{"format":2}"#));
        assert!(bad(r#"{"id":"Upper.Case"}"#));
        assert!(bad(r#"{"id":"a..b"}"#));
        assert!(bad(r#"{"version":"1.0"}"#));
        assert!(bad(r#"{"server":{"address":"evil\"; quit"}}"#));
        assert!(bad(r#"{"branding":{"logo":"../logo.png"}}"#));
        assert!(bad(r#"{"branding":{"logo":"resources/../x"}}"#));
        assert!(bad(r#"{"branding":{"accent":"red"}}"#));
        assert!(bad(r#"{"requires":{"game":["abc"]}}"#));
        assert!(bad(r#"{"patches":[{"offset":"0x10","original":"90","patched":"90 90"}]}"#));
        assert!(bad(r#"{"patches":[{"module":"..\\x.dll","offset":"0x10","original":"90","patched":"91"}]}"#));
    }

    #[test]
    fn versions_and_resources() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert!(parse_version("1.10.0") > parse_version("1.9.9"));
        assert_eq!(resource_rel("resources/img/logo.png"), Some("img/logo.png"));
        assert_eq!(resource_rel("resources/"), None);
        assert_eq!(resource_rel("files/x.png"), None);
    }

    #[test]
    fn pack_round_trips() {
        let p = pack(MINIMAL);
        let again = pack(&p.to_json());
        assert_eq!(again.id, p.id);
        assert!(!p.to_json().contains("patches"));
    }
}

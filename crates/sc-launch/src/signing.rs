//! Pack signatures.
//!
//! A signed pack carries `manifest.json`, listing every file of the pack
//! (`pack.json`, `resources/`, `files/`, `patches/`) with its size and SHA-256,
//! and `manifest.sig`, an ed25519 signature over the manifest's exact bytes.
//! The launcher checks both when a pack is installed, pins the key to the
//! pack id (see [`crate::trust`]), and checks the code-bearing files again
//! before every launch, so a pack can't be swapped or edited behind the
//! player's back.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::{Signature as EdSignature, Signer, SigningKey, Verifier, VerifyingKey};
use sc_profile::{FILES_DIR, GENERATED_FILES, MANIFEST_FILE, PACK_FILE, PATCHES_DIR, RESOURCES_DIR, SIGNATURE_FILE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

const KEY_PREFIX: &str = "ed25519:";
const SECRET_PREFIX: &str = "starforge-pack-secret-v1:";
/// Newest manifest format this build understands.
pub const MANIFEST_FORMAT: u32 = 1;

/// A pack publisher's public key, written `ed25519:<base64>`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    pub fn parse(text: &str) -> Result<Self, String> {
        let b64 = text.trim().strip_prefix(KEY_PREFIX).ok_or_else(|| format!("{text:?}: expected {KEY_PREFIX}<base64>"))?;
        let bytes: [u8; 32] = B64
            .decode(b64)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| format!("{text:?}: not a 32-byte ed25519 key"))?;
        VerifyingKey::from_bytes(&bytes).map(PublicKey).map_err(|e| format!("{text:?}: {e}"))
    }

    /// Short form for people to compare, e.g. `3f2a-91c0-77de-0b14`.
    pub fn fingerprint(&self) -> String {
        let digest = Sha256::digest(self.0.as_bytes());
        let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        hex.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("-")
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{KEY_PREFIX}{}", B64.encode(self.0.as_bytes()))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.fingerprint())
    }
}

/// A publisher's signing key. Kept by the server team, never in a pack.
pub struct SecretKey(SigningKey);

impl SecretKey {
    pub fn generate() -> Result<Self, String> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).map_err(|e| format!("no random source: {e}"))?;
        Ok(SecretKey(SigningKey::from_bytes(&seed)))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let b64 = text.trim().strip_prefix(SECRET_PREFIX).ok_or("not a STAR FORGE pack signing key")?;
        let seed: [u8; 32] = B64
            .decode(b64)
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or("damaged pack signing key")?;
        Ok(SecretKey(SigningKey::from_bytes(&seed)))
    }

    pub fn to_text(&self) -> String {
        format!("{SECRET_PREFIX}{}\n", B64.encode(self.0.to_bytes()))
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }
}

/// One file in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub size: u64,
    pub sha256: String,
}

/// `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    /// Pack id and version, so a signature can't be moved to another pack.
    pub id: String,
    pub version: String,
    /// The key that signed it.
    pub key: String,
    /// Path (with `/` separators) -> entry, for every pack file.
    pub files: BTreeMap<String, Entry>,
}

/// What a pack's signature says, once checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    Unsigned,
    Signed(PublicKey),
}

/// How much of the pack to hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Every file.
    Full,
    /// Everything except `files/`, which is only checked for size: what can
    /// change how the game runs (pack.json, patches, plugins) is always hashed.
    Launch,
}

/// Every file that belongs to the pack, relative, with `/` separators, sorted.
/// Dot files are left out, as when a pack is unpacked, and so are the files
/// the launcher writes itself. A pack whose game files come from a file list
/// (`files_from`) has none of its own: that list is signed separately.
pub fn pack_files(root: &Path, pack: &sc_profile::Pack) -> Result<Vec<String>, String> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<(), String> {
        let rd = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let child = format!("{rel}/{name}");
            match e.file_type() {
                Ok(t) if t.is_dir() => walk(&e.path(), &child, out)?,
                Ok(t) if t.is_file() => out.push(child),
                _ => return Err(format!("{child}: not a regular file")),
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    if root.join(PACK_FILE).is_file() {
        out.push(PACK_FILE.to_string());
    }
    for dir in [RESOURCES_DIR, FILES_DIR, PATCHES_DIR] {
        if dir == FILES_DIR && pack.files_from.is_some() {
            continue;
        }
        walk(&root.join(dir), dir, &mut out)?;
    }
    out.retain(|p| !GENERATED_FILES.iter().any(|g| p.strip_prefix(FILES_DIR).and_then(|r| r.strip_prefix('/')) == Some(g)));
    out.sort();
    Ok(out)
}

fn entry_for(path: &Path) -> Result<Entry, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut h = Sha256::new();
    let size = std::io::copy(&mut f, &mut h).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Entry { size, sha256: h.finalize().iter().map(|b| format!("{b:02x}")).collect() })
}

/// A detached signature over `bytes`, as written to a `.sig` file.
pub fn sign_bytes(key: &SecretKey, bytes: &[u8]) -> String {
    B64.encode(key.0.sign(bytes).to_bytes()) + "\n"
}

/// Checks a detached signature (the text of a `.sig` file) over `bytes`.
pub fn verify_bytes(key: &PublicKey, bytes: &[u8], sig_text: &str) -> Result<(), String> {
    let sig = B64
        .decode(sig_text.trim())
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .map(|b| EdSignature::from_bytes(&b))
        .ok_or("the signature is damaged")?;
    key.0.verify(bytes, &sig).map_err(|_| "the signature does not match".to_string())
}

/// Writes `manifest.json` and `manifest.sig` for the pack in `root`.
pub fn sign(root: &Path, key: &SecretKey) -> Result<Manifest, String> {
    let pack = crate::profiles::read_pack(root)?;
    pack.validate().map_err(|e| e.to_string())?;
    let mut files = BTreeMap::new();
    for rel in pack_files(root, &pack)? {
        files.insert(rel.clone(), entry_for(&root.join(&rel))?);
    }
    let manifest = Manifest {
        format: MANIFEST_FORMAT,
        id: pack.id,
        version: pack.version,
        key: key.public().to_string(),
        files,
    };
    let bytes = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n";
    crate::profiles::write_atomic(&root.join(MANIFEST_FILE), &bytes)?;
    crate::profiles::write_atomic(&root.join(SIGNATURE_FILE), &sign_bytes(key, bytes.as_bytes()))?;
    Ok(manifest)
}

/// Checks the pack in `root` against its manifest. A pack without one is
/// [`Signature::Unsigned`]; a manifest that doesn't verify, or files that
/// don't match it, are errors.
pub fn verify(root: &Path, depth: Depth) -> Result<Signature, String> {
    let (manifest_path, sig_path) = (root.join(MANIFEST_FILE), root.join(SIGNATURE_FILE));
    match (manifest_path.is_file(), sig_path.is_file()) {
        (false, false) => return Ok(Signature::Unsigned),
        (true, true) => {}
        _ => return Err(format!("the pack has only one of {MANIFEST_FILE} and {SIGNATURE_FILE}")),
    }
    let bytes = std::fs::read(&manifest_path).map_err(|e| format!("{MANIFEST_FILE}: {e}"))?;
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("{MANIFEST_FILE}: {e}"))?;
    if manifest.format == 0 || manifest.format > MANIFEST_FORMAT {
        return Err(format!("{MANIFEST_FILE}: format {} is newer than this launcher understands; update it", manifest.format));
    }
    let key = PublicKey::parse(&manifest.key).map_err(|e| format!("{MANIFEST_FILE}: key {e}"))?;
    let sig_text = std::fs::read_to_string(&sig_path).map_err(|e| format!("{SIGNATURE_FILE}: {e}"))?;
    verify_bytes(&key, &bytes, &sig_text)
        .map_err(|_| format!("{SIGNATURE_FILE} does not match {MANIFEST_FILE}: the pack was changed after it was signed"))?;

    let pack = crate::profiles::read_pack(root)?;
    if pack.id != manifest.id || pack.version != manifest.version {
        return Err(format!(
            "{MANIFEST_FILE} was signed for {} {}, not {} {}",
            manifest.id, manifest.version, pack.id, pack.version
        ));
    }
    let present = pack_files(root, &pack)?;
    if let Some(extra) = present.iter().find(|p| !manifest.files.contains_key(*p)) {
        return Err(format!("{extra} is not part of the signed pack"));
    }
    for (rel, want) in &manifest.files {
        if !present.contains(rel) {
            return Err(format!("{rel} is missing from the signed pack"));
        }
        let path = root.join(rel);
        let quick = depth == Depth::Launch && rel.starts_with(&format!("{FILES_DIR}/"));
        let ok = if quick {
            std::fs::metadata(&path).map_err(|e| format!("{rel}: {e}"))?.len() == want.size
        } else {
            entry_for(&path)? == *want
        };
        if !ok {
            return Err(format!("{rel} does not match the signed pack"));
        }
    }
    Ok(Signature::Signed(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sf-sign-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("files/data")).unwrap();
        std::fs::create_dir_all(dir.join("patches")).unwrap();
        std::fs::write(dir.join(PACK_FILE), r#"{"format":1,"id":"org.example.test","name":"Test","version":"1.0.0"}"#).unwrap();
        std::fs::write(dir.join("files/data/a.cfg"), "one").unwrap();
        std::fs::write(dir.join("patches/game.exe.patchset.toml"), "x").unwrap();
        dir
    }

    #[test]
    fn keys_round_trip() {
        let key = SecretKey::generate().unwrap();
        let again = SecretKey::parse(&key.to_text()).unwrap();
        assert_eq!(again.public(), key.public());
        assert_eq!(PublicKey::parse(&key.public().to_string()).unwrap(), key.public());
        assert_eq!(key.public().fingerprint().len(), 19);
        assert!(PublicKey::parse("ed25519:abc").is_err());
        assert!(SecretKey::parse("hello").is_err());
    }

    #[test]
    fn signs_and_detects_changes() {
        let dir = pack_dir("verify");
        assert_eq!(verify(&dir, Depth::Full).unwrap(), Signature::Unsigned);
        let key = SecretKey::generate().unwrap();
        let manifest = sign(&dir, &key).unwrap();
        assert_eq!(manifest.files.len(), 3);
        assert_eq!(verify(&dir, Depth::Full).unwrap(), Signature::Signed(key.public()));

        // Same size, different bytes: only a full check sees it in files/.
        std::fs::write(dir.join("files/data/a.cfg"), "two").unwrap();
        assert!(verify(&dir, Depth::Launch).is_ok());
        assert!(verify(&dir, Depth::Full).is_err());
        std::fs::write(dir.join("files/data/a.cfg"), "one").unwrap();

        // Patches are always hashed.
        std::fs::write(dir.join("patches/game.exe.patchset.toml"), "y").unwrap();
        assert!(verify(&dir, Depth::Launch).is_err());
        std::fs::write(dir.join("patches/game.exe.patchset.toml"), "x").unwrap();

        // The address file the launcher writes doesn't count.
        std::fs::write(dir.join("files/data/address.cfg"), "set net_masterServerAddress \"a:1\"").unwrap();
        assert!(verify(&dir, Depth::Full).is_ok());

        // Extra files can't be slipped in.
        std::fs::write(dir.join("patches/extra.patchset.toml"), "x").unwrap();
        assert!(verify(&dir, Depth::Launch).unwrap_err().contains("not part of the signed pack"));
        std::fs::remove_file(dir.join("patches/extra.patchset.toml")).unwrap();

        // A re-signed manifest with another key verifies as that key, which
        // the trust store then refuses for an already pinned pack.
        let other = SecretKey::generate().unwrap();
        sign(&dir, &other).unwrap();
        assert_eq!(verify(&dir, Depth::Full).unwrap(), Signature::Signed(other.public()));

        // Editing the manifest breaks the signature.
        let text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).unwrap();
        std::fs::write(dir.join(MANIFEST_FILE), text.replace("1.0.0", "1.0.1")).unwrap();
        assert!(verify(&dir, Depth::Full).unwrap_err().contains("changed after it was signed"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

//! Which key may sign which pack.
//!
//! The first signed version of a pack the player installs pins its key to the
//! pack id (trust on first use, kept in `<data dir>/trust.json`). Every later
//! version must be signed by that same key, so a hijacked update address or a
//! look-alike pack can't take over an installed profile. Removing the profile
//! removes the pin.
//!
//! [`KNOWN`] lists publishers the launcher already knows; a pack whose id falls
//! under one of their prefixes must carry their key even on first install.

use crate::signing::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Publishers known to the launcher: (display name, pack id prefix, key).
/// Add an entry here (by pull request) once a server team publishes its key.
pub const KNOWN: &[(&str, &str, &str)] = &[];

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Store {
    #[serde(default)]
    packs: BTreeMap<String, Pin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    key: String,
}

fn file() -> Option<PathBuf> {
    crate::profiles::data_dir().map(|d| d.join("trust.json"))
}

fn load() -> Result<Store, String> {
    let Some(path) = file() else { return Ok(Store::default()) };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn save(store: &Store) -> Result<(), String> {
    let path = file().ok_or("cannot work out the launcher data folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())? + "\n";
    crate::profiles::write_atomic(&path, &text)
}

/// The key pinned to a pack id, if any.
pub fn pinned(id: &str) -> Result<Option<PublicKey>, String> {
    load()?.packs.get(id).map(|p| PublicKey::parse(&p.key)).transpose()
}

pub fn pin(id: &str, key: &PublicKey) -> Result<(), String> {
    let mut store = load()?;
    store.packs.insert(id.to_string(), Pin { key: key.to_string() });
    save(&store)
}

pub fn unpin(id: &str) -> Result<(), String> {
    let mut store = load()?;
    if store.packs.remove(id).is_some() {
        save(&store)?;
    }
    Ok(())
}

/// How far a pack can be trusted, for the install review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// No signature. Allowed, but the player is told.
    Unsigned,
    /// Signed by a key seen for the first time; it gets pinned on install.
    NewKey(PublicKey),
    /// Signed by the key already pinned to this pack.
    Pinned(PublicKey),
    /// Signed by a publisher the launcher knows.
    Known { name: String, key: PublicKey },
}

impl Trust {
    pub fn key(&self) -> Option<PublicKey> {
        match self {
            Trust::Unsigned => None,
            Trust::NewKey(k) | Trust::Pinned(k) | Trust::Known { key: k, .. } => Some(*k),
        }
    }
}

fn known_for(id: &str) -> Option<(&'static str, PublicKey)> {
    KNOWN.iter().find_map(|(name, prefix, key)| {
        let under = id == *prefix || id.strip_prefix(prefix).is_some_and(|rest| rest.starts_with(['.', '-']));
        under.then(|| (*name, PublicKey::parse(key).expect("valid built-in key")))
    })
}

/// Decides whether a pack with this id and signature may be installed (or
/// launched). Errors say why not.
pub fn check(id: &str, signature: Signature) -> Result<Trust, String> {
    let pinned = pinned(id)?;
    let known = known_for(id);
    let key = match signature {
        Signature::Signed(key) => key,
        Signature::Unsigned => {
            if pinned.is_some() || known.is_some() {
                return Err(format!(
                    "{id} is normally signed by its publisher, but this copy isn't; not trusting it"
                ));
            }
            return Ok(Trust::Unsigned);
        }
    };
    if let Some((name, known_key)) = known {
        if key != known_key {
            return Err(format!("{id} belongs to {name}, but this copy is signed by someone else (key {})", key.fingerprint()));
        }
        return Ok(Trust::Known { name: name.to_string(), key });
    }
    match pinned {
        Some(p) if p == key => Ok(Trust::Pinned(key)),
        Some(p) => Err(format!(
            "{id} was signed with key {} when you installed it, but this copy is signed with {}. \
             If its server really changed keys, remove the profile and install the pack again.",
            p.fingerprint(),
            key.fingerprint()
        )),
        None => Ok(Trust::NewKey(key)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::SecretKey;

    #[test]
    fn pins_on_first_use() {
        crate::test_data_dir();
        let (a, b) = (SecretKey::generate().unwrap().public(), SecretKey::generate().unwrap().public());
        let id = "org.example.trust";
        assert_eq!(check(id, Signature::Unsigned).unwrap(), Trust::Unsigned);
        assert_eq!(check(id, Signature::Signed(a)).unwrap(), Trust::NewKey(a));
        pin(id, &a).unwrap();
        assert_eq!(check(id, Signature::Signed(a)).unwrap(), Trust::Pinned(a));
        assert!(check(id, Signature::Signed(b)).is_err());
        assert!(check(id, Signature::Unsigned).is_err(), "can't drop the signature once pinned");
        unpin(id).unwrap();
        assert_eq!(check(id, Signature::Signed(b)).unwrap(), Trust::NewKey(b));
    }
}

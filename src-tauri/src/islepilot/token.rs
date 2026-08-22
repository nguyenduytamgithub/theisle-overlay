//! IslePilot overlay-token storage, encrypted with Windows DPAPI.
//!
//! The overlay token is the account credential for the central
//! `islepilot.eu/api/overlay/*` API — ONE token works across every IslePilot
//! server, so unlike the per-domain cookie store this is a single record.
//! Same sealing rules as cookies.rs: never on disk in plaintext, bound to
//! this Windows user on this machine.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::settings;

use super::cookies::{dpapi_protect, dpapi_unprotect};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OverlayToken {
    pub token: String,
    /// SteamId64 from the auth redirect — identifies "our" marker in map
    /// responses without another decode step.
    pub steam_id: String,
}

fn store_path() -> PathBuf {
    settings::local_dir().join("islepilot_token.bin")
}

pub fn get() -> Option<OverlayToken> {
    let sealed = std::fs::read(store_path()).ok()?;
    // A blob from another machine/user (or a corrupt file) just yields None —
    // the user logs in again.
    dpapi_unprotect(&sealed)
        .ok()
        .and_then(|plain| serde_json::from_slice(&plain).ok())
}

pub fn set(token: &OverlayToken) -> Result<(), String> {
    let plain = serde_json::to_vec(token).map_err(|e| e.to_string())?;
    let sealed = dpapi_protect(&plain)?;
    if let Some(parent) = store_path().parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(store_path(), sealed).map_err(|e| e.to_string())
}

pub fn clear() -> Result<(), String> {
    match std::fs::remove_file(store_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_round_trips_sealed() {
        let secret = OverlayToken {
            token: "opaque-overlay-token".into(),
            steam_id: "76561198000000001".into(),
        };
        let plain = serde_json::to_vec(&secret).unwrap();
        let sealed = dpapi_protect(&plain).unwrap();
        assert_ne!(sealed, plain, "must not be stored in plaintext");
        let back: OverlayToken =
            serde_json::from_slice(&dpapi_unprotect(&sealed).unwrap()).unwrap();
        assert_eq!(back, secret);
    }
}

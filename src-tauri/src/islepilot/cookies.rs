//! IslePilot session-cookie storage, encrypted with Windows DPAPI.
//!
//! The cookie header IS the panel account credential, so it never touches
//! disk in plaintext: the {domain -> cookie} map is serialized to JSON and
//! sealed with CryptProtectData, which binds it to this Windows user on this
//! machine — copying the file elsewhere yields garbage.

use std::collections::HashMap;
use std::path::PathBuf;

use windows::core::PWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
};

use crate::settings;

fn store_path() -> PathBuf {
    settings::local_dir().join("islepilot_cookies.bin")
}

fn dpapi_protect(plain: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, None, None, None, None, 0, &mut output)
            .map_err(|e| e.to_string())?;
        let bytes =
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
        Ok(bytes)
    }
}

fn dpapi_unprotect(sealed: &[u8]) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: sealed.len() as u32,
            pbData: sealed.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(
            &input,
            None::<*mut PWSTR>,
            None,
            None,
            None,
            0,
            &mut output,
        )
        .map_err(|e| e.to_string())?;
        let bytes =
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
        Ok(bytes)
    }
}

fn load_all() -> HashMap<String, String> {
    let Ok(sealed) = std::fs::read(store_path()) else {
        return HashMap::new();
    };
    // A blob from another machine/user (or a corrupt file) just yields an
    // empty map — the user logs in again.
    dpapi_unprotect(&sealed)
        .ok()
        .and_then(|plain| serde_json::from_slice(&plain).ok())
        .unwrap_or_default()
}

fn save_all(map: &HashMap<String, String>) -> Result<(), String> {
    let plain = serde_json::to_vec(map).map_err(|e| e.to_string())?;
    let sealed = dpapi_protect(&plain)?;
    if let Some(parent) = store_path().parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(store_path(), sealed).map_err(|e| e.to_string())
}

pub fn get(domain: &str) -> Option<String> {
    load_all().get(domain).cloned()
}

pub fn set(domain: &str, cookie_header: &str) -> Result<(), String> {
    let mut all = load_all();
    all.insert(domain.to_string(), cookie_header.to_string());
    save_all(&all)
}

pub fn remove(domain: &str) -> Result<(), String> {
    let mut all = load_all();
    all.remove(domain);
    save_all(&all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_round_trips_on_this_machine() {
        let secret = b"next-auth.session-token=abc123; other=1";
        let sealed = dpapi_protect(secret).unwrap();
        assert_ne!(&sealed, secret, "must not be stored in plaintext");
        assert_eq!(dpapi_unprotect(&sealed).unwrap(), secret);
    }
}

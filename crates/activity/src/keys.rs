//! Secrets protected by Windows DPAPI (current-user scope): the database key
//! and the browser extension's shared secret. The files on disk are DPAPI
//! blobs that only this Windows account can decrypt.

use std::fs;
use std::io;
use std::path::Path;

use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

/// The 32-byte SQLCipher key, created on first run.
pub fn db_key(dir: &Path) -> io::Result<[u8; 32]> {
    let key = load_or_create(&dir.join("activity.key"), 32)?;
    Ok(key.try_into().expect("32 bytes"))
}

/// The extension's shared secret: 32 random bytes as hex.
pub fn extension_secret(dir: &Path) -> io::Result<String> {
    let bytes = load_or_create(&dir.join("extension.key"), 32)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Reads the protected secret at `path`, or creates one. Never overwrites a
/// file it can't decrypt: that would orphan whatever the old key protects.
pub fn load_or_create(path: &Path, len: usize) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Ok(blob) => {
            let secret = unprotect(&blob)?;
            if secret.len() != len {
                return Err(io::Error::other(format!("{} holds a key of the wrong length", path.display())));
            }
            Ok(secret)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let mut secret = vec![0u8; len];
            getrandom::fill(&mut secret).map_err(|e| io::Error::other(e.to_string()))?;
            let tmp = path.with_extension("tmp");
            fs::write(&tmp, protect(&secret)?)?;
            fs::rename(&tmp, path)?;
            Ok(secret)
        }
        Err(e) => Err(e),
    }
}

pub fn protect(data: &[u8]) -> io::Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe { CryptProtectData(&input, PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output) }
        .map_err(io::Error::other)?;
    Ok(take_blob(output))
}

pub fn unprotect(blob: &[u8]) -> io::Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: blob.len() as u32, pbData: blob.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe { CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output) }
        .map_err(|e| io::Error::other(format!("can't decrypt key for this Windows user: {e}")))?;
    Ok(take_blob(output))
}

fn take_blob(blob: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize) }.to_vec();
    unsafe { LocalFree(Some(HLOCAL(blob.pbData.cast()))) };
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_round_trips_and_hides_the_secret() {
        let secret = b"0123456789abcdef0123456789abcdef";
        let blob = protect(secret).unwrap();
        assert!(!blob.windows(secret.len()).any(|w| w == secret));
        assert_eq!(unprotect(&blob).unwrap(), secret);
    }

    #[test]
    fn keys_are_created_once_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let first = db_key(dir.path()).unwrap();
        assert_eq!(db_key(dir.path()).unwrap(), first);
        assert_ne!(first, [0; 32]);
        let secret = extension_secret(dir.path()).unwrap();
        assert_eq!(secret.len(), 64);
        assert_eq!(extension_secret(dir.path()).unwrap(), secret);
    }

    #[test]
    fn a_corrupt_key_file_is_an_error_not_a_new_key() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("activity.key"), b"not a dpapi blob").unwrap();
        assert!(db_key(dir.path()).is_err());
        assert_eq!(fs::read(dir.path().join("activity.key")).unwrap(), b"not a dpapi blob");
    }
}

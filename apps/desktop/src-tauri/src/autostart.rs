//! Start with Windows: a per-user `Run` entry (HKCU), never a machine-wide
//! one, and only from an installed build (a dev build would point it at
//! `target/`). The uninstaller removes the entry (`windows/hooks.nsh`).

#[cfg(windows)]
pub fn set(enabled: bool) -> windows::core::Result<()> {
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW};
    use windows::core::w;

    if cfg!(debug_assertions) {
        return Ok(());
    }
    let run = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = w!("Nudgy");
    unsafe {
        // Builds before 0.1.1 wrote a machine-wide entry when run as admin.
        // Removing it needs admin too, so this is best effort.
        let _ = RegDeleteKeyValueW(HKEY_LOCAL_MACHINE, run, name);
        if enabled {
            let exe = std::env::current_exe().map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
            let value: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain([0]).collect();
            RegSetKeyValueW(HKEY_CURRENT_USER, run, name, REG_SZ.0, Some(value.as_ptr().cast()), (value.len() * 2) as u32).ok()
        } else {
            match RegDeleteKeyValueW(HKEY_CURRENT_USER, run, name) {
                ERROR_FILE_NOT_FOUND => Ok(()),
                result => result.ok(),
            }
        }
    }
}

#[cfg(not(windows))]
pub fn set(_enabled: bool) -> Result<(), std::io::Error> {
    Ok(())
}

//! Run Grasp's passive watcher automatically at login.
//!
//! On Windows we register a per-user `Run` key (no admin needed) that launches
//! `grasp watch --silent` through a tiny VBScript shim. The shim runs the
//! console binary with a hidden window, so the always-on capture leaves no
//! stray terminal sitting on the desktop. We drive the registry through the
//! built-in `reg.exe` so there's no extra dependency.

#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::process::Command;

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE_NAME: &str = "Grasp";

/// Enable autostart: write the launcher script and the Run-key entry.
#[cfg(windows)]
pub fn enable(exe: &Path, data_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(data_dir).ok();
    // VBScript: run the watcher with a hidden window (0) and don't wait (False).
    // The triple-quotes wrap the exe path so spaces in it are handled.
    let vbs_path = data_dir.join("autostart.vbs");
    let script = format!(
        "Set s = CreateObject(\"WScript.Shell\")\r\n\
         s.Run \"\"\"{}\"\" watch --silent\", 0, False\r\n",
        exe.display()
    );
    std::fs::write(&vbs_path, script)?;

    // Run-key value: launch the shim via wscript. reg.exe receives the value as
    // a single argument (with embedded quotes) — std handles the escaping.
    let value = format!("wscript.exe \"{}\"", vbs_path.display());
    let status = Command::new("reg")
        .args([
            "add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", &value, "/f",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("`reg add` failed (exit {:?})", status.code());
    }
    Ok(())
}

/// Disable autostart: remove the Run-key entry (the shim file is harmless to leave).
#[cfg(windows)]
pub fn disable() -> anyhow::Result<()> {
    // Ignore failure: a missing value just means it was already off.
    let _ = Command::new("reg")
        .args(["delete", RUN_KEY, "/v", VALUE_NAME, "/f"])
        .status();
    Ok(())
}

/// Is autostart currently registered?
#[cfg(windows)]
pub fn is_enabled() -> bool {
    Command::new("reg")
        .args(["query", RUN_KEY, "/v", VALUE_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Non-Windows stubs: autostart is Windows-only for now (issue #6/#20). ---

#[cfg(not(windows))]
pub fn enable(_exe: &std::path::Path, _data_dir: &std::path::Path) -> anyhow::Result<()> {
    anyhow::bail!("autostart is currently Windows-only")
}

#[cfg(not(windows))]
pub fn disable() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}

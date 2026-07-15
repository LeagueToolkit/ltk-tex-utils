//! Register/unregister the sparse package that surfaces the ltk-tex-utils
//! commands in the Windows 11 modern context menu (see `manifest.rs` for why
//! a package is involved and why registration goes through Developer Mode's
//! loose-manifest path).
//!
//! Registration shells out to Windows PowerShell's `Add-AppxPackage`, the
//! deployment entry point Microsoft documents for installers; going through
//! the WinRT PackageManager API would drag the whole `windows` crate into the
//! CLI for three calls. Everything here is per-user and needs no elevation.

use std::path::{Path, PathBuf};
use std::process::Command;

use colored::Colorize;
use eyre::{Context, Result, bail};

use ltk_tex_handler_shared::{HANDLER_DLL_FILE_NAME, PACKAGE_IDENTITY_NAME};

use super::manifest;

/// First Windows 11 build; the modern context menu exists nowhere below this.
const MIN_WIN11_BUILD: u32 = 22000;

/// Why the modern-menu registration was skipped (not an error: the classic
/// registry verbs still work on their own).
pub enum Skip {
    NotWindows11,
    DllMissing(PathBuf),
    DeveloperModeOff,
}

impl std::fmt::Display for Skip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Skip::NotWindows11 => {
                write!(
                    f,
                    "requires Windows 11 (the modern context menu does not exist here)"
                )
            }
            Skip::DllMissing(path) => write!(
                f,
                "requires the handler DLL next to the executable ({} not found)",
                path.display()
            ),
            Skip::DeveloperModeOff => write!(
                f,
                "requires Windows Developer Mode (Settings > System > For developers) \
                 to register the unsigned menu package"
            ),
        }
    }
}

fn read_hklm_dword(subkey: &str, value: &str) -> Option<u32> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(subkey)
        .and_then(|k| k.get_value::<u32, _>(value))
        .ok()
}

fn windows_build() -> u32 {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
        .and_then(|k| k.get_value::<String, _>("CurrentBuildNumber"))
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

fn developer_mode_enabled() -> bool {
    read_hklm_dword(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock",
        "AllowDevelopmentWithoutDevLicense",
    ) == Some(1)
}

/// Check the environment supports the modern menu at all.
fn applicable(exe_dir: &Path) -> Option<Skip> {
    if windows_build() < MIN_WIN11_BUILD {
        return Some(Skip::NotWindows11);
    }
    let dll = exe_dir.join(HANDLER_DLL_FILE_NAME);
    if !dll.is_file() {
        return Some(Skip::DllMissing(dll));
    }
    if !developer_mode_enabled() {
        return Some(Skip::DeveloperModeOff);
    }
    None
}

/// Stable home for the registered manifest. Not the exe directory: that may be
/// read-only (Program Files), and Windows keeps referencing the manifest's
/// location for as long as the package stays registered, so a temp dir is out.
fn manifest_path() -> Result<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| eyre::eyre!("LOCALAPPDATA is not set"))?;
    Ok(base
        .join("LeagueToolkit")
        .join("ltk-tex-utils-shell")
        .join("AppxManifest.xml"))
}

/// Run a PowerShell command line, capturing output. Uses Windows PowerShell
/// (always present, and its Appx module needs no compatibility shim).
fn powershell(command: &str) -> Result<std::process::Output> {
    Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            command,
        ])
        .output()
        .wrap_err("failed to launch powershell.exe")
}

/// Single-quote a value for PowerShell (single quotes suppress all expansion;
/// embedded ones are doubled).
pub(crate) fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Register the sparse package pointing at `exe_dir`. Returns `Ok(Some(skip))`
/// when the environment can't host the modern menu.
pub fn install(exe_dir: &Path) -> Result<Option<Skip>> {
    if let Some(skip) = applicable(exe_dir) {
        return Ok(Some(skip));
    }

    let manifest_path = manifest_path()?;
    if let Some(dir) = manifest_path.parent() {
        std::fs::create_dir_all(dir)
            .wrap_err_with(|| format!("failed to create {}", dir.display()))?;
    }
    std::fs::write(&manifest_path, manifest::appx_manifest())
        .wrap_err_with(|| format!("failed to write {}", manifest_path.display()))?;

    // Re-registering the same version is rejected, so drop any prior
    // registration first; a failed removal surfaces via the Add below.
    let command = format!(
        "$ErrorActionPreference = 'Stop'; \
         Get-AppxPackage -Name {name} -ErrorAction SilentlyContinue | Remove-AppxPackage; \
         Add-AppxPackage -Register {path} -ExternalLocation {dir}",
        name = ps_quote(PACKAGE_IDENTITY_NAME),
        path = ps_quote(&manifest_path.display().to_string()),
        dir = ps_quote(&exe_dir.display().to_string()),
    );
    let output = powershell(&command)?;

    if !output.status.success() {
        bail!(
            "Add-AppxPackage failed:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(None)
}

/// Remove the sparse package if it is registered. Returns whether it was.
pub fn uninstall() -> Result<bool> {
    if registered_version()?.is_none() {
        return Ok(false);
    }
    let command = format!(
        "$ErrorActionPreference = 'Stop'; \
         Get-AppxPackage -Name {name} | Remove-AppxPackage",
        name = ps_quote(PACKAGE_IDENTITY_NAME),
    );
    let output = powershell(&command)?;
    if !output.status.success() {
        bail!(
            "Remove-AppxPackage failed:\n{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    if let Ok(path) = manifest_path() {
        let _ = std::fs::remove_file(path);
    }
    Ok(true)
}

/// Version of the registered package, if any.
pub fn registered_version() -> Result<Option<String>> {
    let command = format!(
        "(Get-AppxPackage -Name {name} -ErrorAction SilentlyContinue).Version",
        name = ps_quote(PACKAGE_IDENTITY_NAME),
    );
    let output = powershell(&command)?;
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!version.is_empty()).then_some(version))
}

/// One status line for `shell status`, mirroring the row style used there.
/// Takes the pre-fetched [`registered_version`] so the caller can reuse it.
pub fn print_status(exe_dir: &Path, registered: Option<&str>) -> Result<()> {
    let good = "\u{2713}".green().bold();
    let bad = "\u{2717}".red().bold();
    let warn = "!".yellow().bold();

    match registered {
        Some(version) => {
            let expected = manifest::package_version();
            if version == expected {
                println!(
                    "  {good} modern menu (Win11)  registered {}",
                    format!("(v{version})").dimmed()
                );
            } else {
                println!(
                    "  {warn} modern menu (Win11)  registered {}",
                    format!("(v{version}, current is v{expected} - reinstall to update)").yellow()
                );
            }
        }
        None => match applicable(exe_dir) {
            Some(skip) => println!("  {warn} modern menu (Win11)  not installed: {skip}"),
            None => println!("  {bad} modern menu (Win11)  {}", "not installed".dimmed()),
        },
    }
    Ok(())
}

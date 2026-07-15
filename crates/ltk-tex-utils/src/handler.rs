//! Install/uninstall the `.tex` thumbnail + preview shell handler DLL.
//!
//! This drives the same COM DLL that `regsvr32` registers, but adds an opt-in
//! `--override` mode. In override mode the DLL takes over the thumbnail/preview
//! slots from whichever application currently owns the `.tex` ProgID (backing up
//! the prior association so `uninstall` restores it). Because `.tex` is also the
//! LaTeX source extension, override is gated behind a warning and confirmation.
//!
//! The toggle reaches the DLL through the `LTK_TEX_HANDLER_OVERRIDE` environment
//! variable: `DllRegisterServer` (invoked here via `regsvr32`) reads it, because
//! COM registration entrypoints take no arguments. Registration itself lives in
//! the DLL so it writes its own path into `InprocServer32`.

use eyre::Result;

/// Actions for the `handler` subcommand.
#[derive(clap::Subcommand, Debug)]
pub enum HandlerAction {
    /// Register the `.tex` thumbnail/preview handler (requires an elevated shell)
    Install {
        /// Take over `.tex` previews even if another application already owns the
        /// type. WARNING: `.tex` is also the LaTeX source extension - see the note
        /// printed before installation.
        #[arg(long = "override")]
        override_existing: bool,

        /// Skip the confirmation prompt shown for `--override`.
        #[arg(long)]
        yes: bool,
    },
    /// Unregister the handler and restore any association it overrode
    Uninstall,
    /// Show whether the handler is registered, in which mode, and where the DLL lives
    Status,
}

pub fn run(action: &HandlerAction) -> Result<()> {
    #[cfg(windows)]
    {
        match action {
            HandlerAction::Install {
                override_existing,
                yes,
            } => windows_impl::install(*override_existing, *yes),
            HandlerAction::Uninstall => windows_impl::uninstall(),
            HandlerAction::Status => windows_impl::status(),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        eyre::bail!("the thumbnail handler is only supported on Windows");
    }
}

#[cfg(windows)]
mod windows_impl {
    use colored::Colorize;
    use eyre::{Context, Result, bail};
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE};

    // Registry identifiers and the override toggle, shared with the handler DLL
    // so the two never drift out of sync.
    use ltk_tex_handler_shared::{
        CLSID_TEX_THUMB_HANDLER, IID_ITHUMBNAILPROVIDER, OVERRIDE_BACKUP_KEY, OVERRIDE_ENV,
    };

    const DLL_NAME: &str = "ltk_tex_thumb_handler.dll";

    /// Locate the handler DLL: next to this executable first (source builds and
    /// side-by-side installs), then the default install directory used by the
    /// `install-thumbnail-handler.ps1` script.
    fn find_dll() -> Result<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            candidates.push(dir.join(DLL_NAME));
        }
        if let Ok(program_files) = std::env::var("ProgramFiles") {
            candidates.push(
                PathBuf::from(program_files)
                    .join("LeagueToolkit")
                    .join("ltk-tex-thumb-handler")
                    .join(DLL_NAME),
            );
        }

        for path in &candidates {
            if path.is_file() {
                return Ok(path.clone());
            }
        }

        bail!(
            "could not find {DLL_NAME}. Looked in:\n{}\n\
             Install it with the thumbnail-handler script, or place the DLL next to this executable.",
            candidates
                .iter()
                .map(|p| format!("  {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Print the LaTeX warning and ask for confirmation. Returns whether to proceed.
    fn confirm_override() -> Result<bool> {
        println!("WARNING: '.tex' is also the LaTeX source-file extension.");
        println!(
            "Override mode will take over .tex thumbnails and previews from whatever\n\
             application currently owns the type (e.g. a LaTeX editor or another tool).\n\
             The double-click 'open' association is left untouched, and 'handler uninstall'\n\
             restores the previous thumbnail/preview owner."
        );
        print!("Proceed with override install? [y/N] ");
        std::io::stdout().flush().ok();

        let mut answer = String::new();
        std::io::stdin()
            .read_line(&mut answer)
            .wrap_err("failed to read confirmation")?;
        Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ))
    }

    /// Run `regsvr32` against the DLL, optionally enabling override mode.
    fn run_regsvr32(dll: &PathBuf, unregister: bool, override_existing: bool) -> Result<()> {
        let mut cmd = Command::new("regsvr32.exe");
        cmd.arg("/s");
        if unregister {
            cmd.arg("/u");
        }
        cmd.arg(dll);
        if override_existing {
            cmd.env(OVERRIDE_ENV, "1");
        }

        let status = cmd.status().wrap_err("failed to launch regsvr32.exe")?;

        if !status.success() {
            bail!(
                "regsvr32 exited with {}. This usually means the command was not run from an \
                 elevated (Administrator) terminal - registering the handler writes to HKLM/HKCR.",
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "an unknown code".into())
            );
        }
        Ok(())
    }

    pub fn install(override_existing: bool, yes: bool) -> Result<()> {
        let dll = find_dll()?;

        if override_existing && !yes && !confirm_override()? {
            println!("Aborted; nothing was changed.");
            return Ok(());
        }

        run_regsvr32(&dll, false, override_existing)?;

        println!(
            "Registered the .tex thumbnail/preview handler{}.",
            if override_existing {
                " in override mode"
            } else {
                ""
            }
        );
        println!("(using {})", dll.display());
        println!("You may need to restart Windows Explorer for thumbnails to refresh.");
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let dll = find_dll()?;
        // Unregistering triggers DllUnregisterServer, which also restores any
        // association that override mode took over.
        run_regsvr32(&dll, true, false)?;
        println!("Unregistered the .tex thumbnail/preview handler.");
        println!("(using {})", dll.display());
        Ok(())
    }

    pub fn status() -> Result<()> {
        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // Marker + padded field label, so the value columns line up.
        const FIELD_W: usize = 14;
        let good = || "\u{2713}".green().bold();
        let bad = || "\u{2717}".red().bold();
        let warn = || "!".yellow().bold();

        println!("{}", ".tex thumbnail / preview handler".bold());
        println!();

        // Is the COM server registered at all?
        let registered = hkcr
            .open_subkey(format!("CLSID\\{CLSID_TEX_THUMB_HANDLER}"))
            .is_ok();
        if registered {
            println!("  {} {:<FIELD_W$}  registered", good(), "COM server");
        } else {
            println!(
                "  {} {:<FIELD_W$}  {}",
                bad(),
                "COM server",
                "not registered".dimmed()
            );
        }

        // Extension-level thumbnail slot.
        match hkcr
            .open_subkey(format!(".tex\\ShellEx\\{IID_ITHUMBNAILPROVIDER}"))
            .and_then(|k| k.get_value::<String, _>(""))
        {
            Ok(clsid) if clsid.eq_ignore_ascii_case(CLSID_TEX_THUMB_HANDLER) => {
                println!("  {} {:<FIELD_W$}  ours", good(), "thumbnail slot");
            }
            Ok(clsid) => println!(
                "  {} {:<FIELD_W$}  {} {}",
                warn(),
                "thumbnail slot",
                clsid.yellow(),
                "(not ours)".dimmed()
            ),
            Err(_) => println!(
                "  {} {:<FIELD_W$}  {}",
                bad(),
                "thumbnail slot",
                "none".dimmed()
            ),
        }

        // Override mode leaves a backup key naming the ProgID it took over.
        match hklm.open_subkey(OVERRIDE_BACKUP_KEY) {
            Ok(backup) => {
                let progid: String = backup.get_value("ProgId").unwrap_or_default();
                println!(
                    "  {} {:<FIELD_W$}  active {}",
                    good(),
                    "override",
                    format!("(took over ProgID '{}')", progid.trim()).dimmed()
                );
            }
            Err(_) => println!(
                "    {:<FIELD_W$}  {}",
                "override",
                "inactive".dimmed()
            ),
        }

        match find_dll() {
            Ok(dll) => println!(
                "  {} {:<FIELD_W$}  {}",
                good(),
                "DLL",
                dll.display().to_string().dimmed()
            ),
            Err(_) => println!("  {} {:<FIELD_W$}  {}", bad(), "DLL", "not found".dimmed()),
        }
        Ok(())
    }
}

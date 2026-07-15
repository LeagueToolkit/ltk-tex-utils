//! Install/uninstall the `.tex` thumbnail + preview shell handler DLL.
//!
//! Install copies the DLL into Program Files and registers that copy (see
//! `installed_dll_path` for why). When another
//! application competes for `.tex` - a foreign ProgID owning the extension, or
//! OpenWithProgids entries that steal Explorer's Type column - install takes
//! over the contested slots (the double-click 'open' association is never
//! touched), backing up the prior state so `uninstall` restores it.
//! `--no-override` opts out of the takeover.
//!
//! The toggle reaches the DLL through the `LTK_TEX_HANDLER_OVERRIDE` environment
//! variable: `DllRegisterServer` (invoked here via `regsvr32`) reads it, because
//! COM registration entrypoints take no arguments. Registration itself lives in
//! the DLL so it writes its own path into `InprocServer32`.

use eyre::Result;

/// Actions for the `handler` subcommand.
#[derive(clap::Subcommand, Debug)]
pub enum HandlerAction {
    /// Register the `.tex` thumbnail/preview handler (shows a UAC prompt when
    /// the terminal isn't elevated). If another application owns `.tex`
    /// previews, takes them over (reversible via `handler uninstall`).
    Install {
        /// Never take over an existing `.tex` preview owner; register alongside it only
        #[arg(long = "no-override")]
        no_override: bool,
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
            HandlerAction::Install { no_override } => windows_impl::install(*no_override),
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
    use std::path::PathBuf;
    use std::process::Command;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_WRITE};

    // Registry identifiers and the override toggle, shared with the handler DLL
    // so the two never drift out of sync.
    use ltk_tex_handler_shared::{
        CLSID_TEX_THUMB_HANDLER, IID_ITHUMBNAILPROVIDER, OVERRIDE_BACKUP_KEY,
        OVERRIDE_BACKUP_OPENWITH_SUBKEY, OVERRIDE_ENV, PROGID_TEX,
    };

    use crate::shell::modern::ps_quote;

    const DLL_NAME: &str = "ltk_tex_thumb_handler.dll";

    /// Machine-wide home of the *registered* DLL. The COM registration lives in
    /// HKLM, so every account - and the file dialogs of elevated processes,
    /// which load shell extensions in-proc - resolves it; the DLL it points at
    /// must therefore not sit in a user-writable location like the exe
    /// directory under `%LOCALAPPDATA%`, or any user-level process could plant
    /// code that runs elevated. Install copies the DLL here (admin-writable
    /// Program Files) and registers the copy. Same directory the retired
    /// `install-thumbnail-handler.ps1` script used, so legacy installs are
    /// managed in place.
    fn installed_dll_path() -> Result<PathBuf> {
        let program_files = std::env::var("ProgramFiles").wrap_err("ProgramFiles is not set")?;
        Ok(PathBuf::from(program_files)
            .join("LeagueToolkit")
            .join("ltk-tex-thumb-handler")
            .join(DLL_NAME))
    }

    /// Locate a handler DLL to work with: next to this executable first (where
    /// the quick-install script and source builds put it), then the installed
    /// copy in Program Files.
    fn find_dll() -> Result<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            candidates.push(dir.join(DLL_NAME));
        }
        if let Ok(installed) = installed_dll_path() {
            candidates.push(installed);
        }

        for path in &candidates {
            if path.is_file() {
                return Ok(path.clone());
            }
        }

        bail!(
            "could not find {DLL_NAME}. Looked in:\n{}\n\
             Re-run the Windows quick-install script to download it, or place the DLL next to this executable.",
            candidates
                .iter()
                .map(|p| format!("  {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    /// Who currently holds the `.tex` slots that matter (thumbnails, previews,
    /// and the Type column).
    enum TexOwnership {
        /// `.tex` has no ProgID (or our own) and no competing OpenWith entries -
        /// our registration wins on its own.
        Unowned,
        /// Our override is already in place (backup key present, or the ProgID
        /// slots already point at our handler).
        OverrideActive,
        /// Another application's ProgID owns `.tex` (named here).
        Foreign(String),
        /// Foreign OpenWithProgids entries (named here) steal Explorer's Type
        /// column until taken over.
        TypeNameCompetitor(String),
    }

    /// Inspect the registry to see whether taking over `.tex` is needed at all.
    fn tex_ownership() -> TexOwnership {
        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        if hklm.open_subkey(OVERRIDE_BACKUP_KEY).is_ok() {
            return TexOwnership::OverrideActive;
        }

        let progid = hkcr
            .open_subkey(".tex")
            .and_then(|k| k.get_value::<String, _>(""))
            .unwrap_or_default();
        let progid = progid.trim().to_string();
        if !progid.is_empty() && !progid.eq_ignore_ascii_case(PROGID_TEX) {
            let slot_owner = hkcr
                .open_subkey(format!("{progid}\\ShellEx\\{IID_ITHUMBNAILPROVIDER}"))
                .and_then(|k| k.get_value::<String, _>(""))
                .unwrap_or_default();
            if slot_owner.eq_ignore_ascii_case(CLSID_TEX_THUMB_HANDLER) {
                return TexOwnership::OverrideActive;
            }
            return TexOwnership::Foreign(progid);
        }

        let competitors: Vec<String> = hkcr
            .open_subkey(".tex\\OpenWithProgids")
            .map(|k| {
                k.enum_values()
                    .filter_map(|v| v.ok())
                    .map(|(name, _)| name)
                    .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(PROGID_TEX))
                    .collect()
            })
            .unwrap_or_default();
        if !competitors.is_empty() {
            return TexOwnership::TypeNameCompetitor(competitors.join(", "));
        }

        TexOwnership::Unowned
    }

    /// Whether this process can write HKLM. (Un)registration needs it, and the
    /// check also guards uninstall: the DLL's removals are best-effort, so
    /// regsvr32 alone can't be trusted to fail loudly when unelevated.
    fn is_elevated() -> bool {
        RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags("SOFTWARE", KEY_WRITE)
            .is_ok()
    }

    /// Set in the environment of the elevated relaunch so it can never chain
    /// into another elevation attempt.
    const ELEVATED_ENV: &str = "LTK_TEX_UTILS_ELEVATED";

    /// Re-run the current command elevated instead of failing: `Start-Process
    /// -Verb RunAs` shows the standard UAC consent prompt and starts an
    /// elevated copy. Elevation severs the console connection, so the copy
    /// runs in a *hidden* window through `cmd /c` with everything redirected
    /// to a temp file (cmd redirection is raw bytes; Windows PowerShell's `>`
    /// would write UTF-16), and the file is printed here once it exits - the
    /// user never sees or interacts with the elevated window. The exit code is
    /// forwarded so scripts still observe failures. Exit code 1223 is Windows'
    /// ERROR_CANCELLED: the user declined the UAC prompt.
    fn relaunch_elevated() -> Result<()> {
        use std::io::IsTerminal;

        // Without a user at a console there is nobody to answer the UAC
        // prompt, and an elevated run that still can't write HKLM must not
        // chain into another prompt.
        if std::env::var_os(ELEVATED_ENV).is_some() || !std::io::stdin().is_terminal() {
            bail!(
                "this command writes to HKLM and must be run from an elevated \
                 (Administrator) terminal"
            );
        }

        let exe = std::env::current_exe().wrap_err("failed to resolve the current executable")?;
        // Forward the original arguments minus any `--pause`: a pause prompt
        // in the hidden console would wait for an Enter that can never come.
        let mut args: Vec<String> = Vec::new();
        let mut raw = std::env::args().skip(1);
        while let Some(arg) = raw.next() {
            if arg == "--pause" {
                raw.next();
                continue;
            }
            if arg.starts_with("--pause=") {
                continue;
            }
            args.push(arg);
        }
        let log =
            std::env::temp_dir().join(format!("ltk-tex-utils-elevated-{}.log", std::process::id()));

        println!(
            "{} administrator access is needed - requesting elevation (UAC)...",
            "note:".yellow().bold()
        );

        let cmd_line = format!(
            "/d /s /c \"\"{exe}\" {args} > \"{log}\" 2>&1\"",
            exe = exe.display(),
            args = args
                .iter()
                .map(|a| format!("\"{a}\""))
                .collect::<Vec<_>>()
                .join(" "),
            log = log.display(),
        );
        let command = format!(
            "$ErrorActionPreference = 'Stop'; \
             try {{ $p = Start-Process -FilePath 'cmd.exe' -ArgumentList {cmd_line} \
             -Verb RunAs -WindowStyle Hidden -Wait -PassThru }} \
             catch {{ exit 1223 }}; \
             exit $p.ExitCode",
            cmd_line = ps_quote(&cmd_line),
        );
        let status = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &command,
            ])
            .env(ELEVATED_ENV, "1")
            .status()
            .wrap_err("failed to launch powershell.exe")?;

        if let Ok(output) = std::fs::read_to_string(&log) {
            print!("{output}");
        }
        let _ = std::fs::remove_file(&log);

        match status.code() {
            Some(0) => Ok(()),
            Some(1223) => bail!("the elevation request was declined"),
            _ => bail!("the elevated command failed"),
        }
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

    /// Copy the DLL into its Program Files home. Explorer keeps a registered
    /// DLL loaded, which blocks overwriting but not renaming - a loaded copy is
    /// moved aside and the leftover cleaned up on the next run.
    fn stage_installed_dll(source: &PathBuf, dest: &PathBuf) -> Result<()> {
        let dir = dest.parent().expect("installed DLL path has a parent");
        std::fs::create_dir_all(dir)
            .wrap_err_with(|| format!("failed to create {}", dir.display()))?;

        let stale = dest.with_extension("dll.old");
        let _ = std::fs::remove_file(&stale);

        if std::fs::copy(source, dest).is_err() {
            if dest.is_file() {
                std::fs::rename(dest, &stale)
                    .wrap_err_with(|| format!("failed to move aside {}", dest.display()))?;
            }
            std::fs::copy(source, dest).wrap_err_with(|| {
                format!("failed to copy {} to {}", source.display(), dest.display())
            })?;
        }
        Ok(())
    }

    pub fn install(no_override: bool) -> Result<()> {
        if !is_elevated() {
            return relaunch_elevated();
        }

        let source = find_dll()?;
        let dll = installed_dll_path()?;
        if source != dll {
            stage_installed_dll(&source, &dll)?;
        }

        // Unless the user opted out with --no-override, the DLL takes over
        // whatever currently competes for `.tex`: a foreign ProgID's
        // thumbnail/preview slots, and any OpenWithProgids entries that would
        // steal the Type column. Everything is backed up and undone by
        // uninstall.
        let ownership = tex_ownership();
        if no_override && matches!(ownership, TexOwnership::OverrideActive) {
            println!(
                "{} an earlier override is still active; {} restores the previous owner.",
                "note:".yellow().bold(),
                "handler uninstall".cyan()
            );
            println!();
        }

        run_regsvr32(&dll, false, !no_override)?;

        if !no_override {
            match &ownership {
                TexOwnership::Foreign(progid) => {
                    println!(
                        "Took over .tex thumbnails & previews from {} ({} restores it;",
                        format!("'{progid}'").cyan(),
                        "handler uninstall".cyan()
                    );
                    println!("the double-click 'open' association is untouched).");
                    println!();
                }
                TexOwnership::TypeNameCompetitor(names) => {
                    println!(
                        "Took over the .tex type name from {} ({} restores it;",
                        format!("'{names}'").cyan(),
                        "handler uninstall".cyan()
                    );
                    println!("the app stays available in the Open With menu).");
                    println!();
                }
                TexOwnership::Unowned | TexOwnership::OverrideActive => {}
            }
        }

        let took_over = !no_override && !matches!(ownership, TexOwnership::Unowned);
        println!(
            "{} Registered the .tex thumbnail & preview handler{}",
            "\u{2713}".green().bold(),
            if took_over {
                format!(" {}", "(override mode)".yellow())
            } else {
                String::new()
            }
        );
        println!("  {}", dll.display().to_string().dimmed());
        println!();
        println!(
            "{} you may need to restart Windows Explorer for thumbnails to refresh",
            "note:".yellow().bold()
        );
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        if !is_elevated() {
            return relaunch_elevated();
        }
        let installed = installed_dll_path()?;
        let dll = if installed.is_file() {
            installed.clone()
        } else {
            find_dll()?
        };
        // Unregistering triggers DllUnregisterServer, which also restores any
        // association that override mode took over.
        run_regsvr32(&dll, true, false)?;

        // Best-effort removal of the installed copy; a copy still loaded by
        // Explorer is renamed aside instead and cleaned up on the next install.
        if dll == installed {
            if std::fs::remove_file(&dll).is_err() && dll.is_file() {
                let _ = std::fs::rename(&dll, dll.with_extension("dll.old"));
            }
            if let Some(dir) = dll.parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }

        println!(
            "{} Unregistered the .tex thumbnail & preview handler",
            "\u{2713}".green().bold()
        );
        println!("  {}", dll.display().to_string().dimmed());
        Ok(())
    }

    /// One aligned status row: `  <marker> <label>  <value>`.
    fn print_row(marker: impl std::fmt::Display, label: &str, value: impl std::fmt::Display) {
        const FIELD_W: usize = 14;
        println!("  {marker} {label:<FIELD_W$}  {value}");
    }

    /// Human-readable list of what the override backup says install took over
    /// (the prior ProgID and/or removed OpenWithProgids entries).
    fn override_takeovers() -> Vec<String> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let mut takeovers = Vec::new();

        let progid: String = hklm
            .open_subkey(OVERRIDE_BACKUP_KEY)
            .and_then(|k| k.get_value("ProgId"))
            .unwrap_or_default();
        if !progid.trim().is_empty() {
            takeovers.push(format!("took over ProgID '{}'", progid.trim()));
        }

        let openwith: Vec<String> = ["HKCU", "HKLM"]
            .iter()
            .filter_map(|hive| {
                hklm.open_subkey(format!(
                    "{OVERRIDE_BACKUP_KEY}\\{OVERRIDE_BACKUP_OPENWITH_SUBKEY}\\{hive}"
                ))
                .ok()
            })
            .flat_map(|k| {
                k.enum_values()
                    .filter_map(|v| v.ok())
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            })
            .collect();
        if !openwith.is_empty() {
            takeovers.push(format!("took over type name from {}", openwith.join(", ")));
        }

        takeovers
    }

    pub fn status() -> Result<()> {
        let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
        let good = "\u{2713}".green().bold();
        let bad = "\u{2717}".red().bold();
        let warn = "!".yellow().bold();

        println!("{}", ".tex thumbnail / preview handler".bold());
        println!();

        match hkcr
            .open_subkey(format!("CLSID\\{CLSID_TEX_THUMB_HANDLER}\\InprocServer32"))
            .and_then(|k| k.get_value::<String, _>(""))
        {
            Ok(path) => print_row(
                &good,
                "COM server",
                format!("registered  {}", path.dimmed()),
            ),
            Err(_) => print_row(&bad, "COM server", "not registered".dimmed()),
        }

        // Extension-level thumbnail slot.
        match hkcr
            .open_subkey(format!(".tex\\ShellEx\\{IID_ITHUMBNAILPROVIDER}"))
            .and_then(|k| k.get_value::<String, _>(""))
        {
            Ok(clsid) if clsid.eq_ignore_ascii_case(CLSID_TEX_THUMB_HANDLER) => {
                print_row(&good, "thumbnail slot", "ours");
            }
            Ok(clsid) => print_row(
                &warn,
                "thumbnail slot",
                format!("{} {}", clsid.yellow(), "(not ours)".dimmed()),
            ),
            Err(_) => print_row(&bad, "thumbnail slot", "none".dimmed()),
        }

        match tex_ownership() {
            TexOwnership::OverrideActive => {
                let takeovers = override_takeovers();
                let detail = if takeovers.is_empty() {
                    "(ProgID slots point at us)".to_string()
                } else {
                    format!("({})", takeovers.join("; "))
                };
                print_row(&good, "override", format!("active {}", detail.dimmed()));
            }
            TexOwnership::Unowned => print_row(
                " ",
                "override",
                "not needed (nothing else competes for .tex)".dimmed(),
            ),
            TexOwnership::Foreign(progid) => print_row(
                &warn,
                "override",
                format!(
                    "available {}",
                    format!("('{progid}' owns .tex - reinstall to take over)").dimmed()
                ),
            ),
            TexOwnership::TypeNameCompetitor(names) => print_row(
                &warn,
                "override",
                format!(
                    "available {}",
                    format!("('{names}' steals the .tex type name - reinstall to take over)")
                        .dimmed()
                ),
            ),
        }

        match find_dll() {
            Ok(dll) => print_row(&good, "DLL", dll.display().to_string().dimmed()),
            Err(_) => print_row(&bad, "DLL", "not found".dimmed()),
        }
        Ok(())
    }
}

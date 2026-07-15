//! Windows Explorer shell integration: register/unregister right-click context-menu
//! entries for `.tex`, `.dds`, and `.png` files and for folders.
//!
//! Entries are written under `HKEY_CURRENT_USER\Software\Classes` so no administrator
//! rights are required. File verbs are attached to `SystemFileAssociations\<ext>` so they
//! apply regardless of which application owns the extension's ProgID.
//!
//! All entries are grouped under a single cascading **LTK Toolz** submenu. This is done
//! with a parent verb carrying `MUIVerb` and an empty `SubCommands` value; Explorer then
//! enumerates the sub-verbs from a nested `shell` subkey.
//!
//! On Windows 11 the packaged menu (see `modern`/`manifest`) is registered *instead*,
//! because it serves both the modern menu and "Show more options"; the registry verbs
//! here are the fallback when the package can't register, or an explicit choice via
//! `shell install --classic`.

use eyre::Result;

#[cfg(windows)]
mod manifest;
#[cfg(windows)]
pub(crate) mod modern;

/// Actions for the `shell` subcommand.
#[derive(clap::Subcommand, Debug)]
pub enum ShellAction {
    /// Register the ltk-tex-utils Explorer context-menu entries (per-user, no admin required)
    Install {
        /// Install the classic registry menu even where the Windows 11 packaged menu is
        /// available (renders only under "Show more options", but stays pinned to the top
        /// of the .tex context menu)
        #[arg(long)]
        classic: bool,
    },
    /// Remove the ltk-tex-utils Explorer context-menu entries
    Uninstall,
    /// Show whether the context-menu entries are installed and where they point
    Status,
}

pub fn run(action: &ShellAction) -> Result<()> {
    #[cfg(windows)]
    {
        match action {
            ShellAction::Install { classic } => windows_impl::install(*classic),
            ShellAction::Uninstall => windows_impl::uninstall(),
            ShellAction::Status => windows_impl::status(),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        eyre::bail!("shell integration is only supported on Windows");
    }
}

#[cfg(windows)]
mod windows_impl {
    use colored::Colorize;
    use eyre::{Context, Result};
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    /// The registry class a menu is attached to.
    #[derive(Copy, Clone)]
    enum VerbRoot {
        /// `.tex` files.
        Tex,
        /// `.dds` files.
        Dds,
        /// `.png` files.
        Png,
        /// Directories.
        Directory,
    }

    impl VerbRoot {
        fn class(self) -> &'static str {
            match self {
                VerbRoot::Tex => "SystemFileAssociations\\.tex",
                VerbRoot::Dds => "SystemFileAssociations\\.dds",
                VerbRoot::Png => "SystemFileAssociations\\.png",
                VerbRoot::Directory => "Directory",
            }
        }

        fn describe(self) -> &'static str {
            match self {
                VerbRoot::Tex => ".tex",
                VerbRoot::Dds => ".dds",
                VerbRoot::Png => ".png",
                VerbRoot::Directory => "folders",
            }
        }
    }

    /// A single entry inside the cascading [`MENU_LABEL`] submenu. `command` uses `{exe}`
    /// as a placeholder for the executable path; Explorer substitutes `%1` with the clicked
    /// item. With multiple items selected, Explorer invokes the verb once per item.
    struct SubVerb {
        key: &'static str,
        label: &'static str,
        command: &'static str,
    }

    /// A cascading [`MENU_LABEL`] submenu attached to one registry class. The parent verb
    /// holds the sub-verbs in a nested `shell` key.
    struct Menu {
        root: VerbRoot,
        /// Force the submenu to the top of the context menu.
        position_top: bool,
        subverbs: &'static [SubVerb],
    }

    use ltk_tex_handler_shared::MENU_LABEL;

    /// Parent key name of the cascading submenu (under `<class>\shell`).
    const MENU_KEY: &str = "ltktexutils";

    const MENUS: &[Menu] = &[
        Menu {
            root: VerbRoot::Tex,
            position_top: true,
            subverbs: &[
                SubVerb {
                    key: "topng",
                    label: "Convert to PNG",
                    command: "\"{exe}\" --pause on-error decode --format png \"%1\"",
                },
                SubVerb {
                    key: "todds",
                    label: "Convert to DDS",
                    command: "\"{exe}\" --pause on-error decode --format dds \"%1\"",
                },
            ],
        },
        Menu {
            root: VerbRoot::Dds,
            position_top: false,
            subverbs: &[SubVerb {
                key: "totex",
                label: "Convert to TEX",
                command: "\"{exe}\" --pause on-error encode \"%1\"",
            }],
        },
        Menu {
            root: VerbRoot::Png,
            position_top: false,
            subverbs: &[SubVerb {
                key: "totex",
                label: "Convert to TEX",
                command: "\"{exe}\" --pause on-error encode \"%1\"",
            }],
        },
        Menu {
            root: VerbRoot::Directory,
            position_top: false,
            subverbs: &[
                SubVerb {
                    key: "alltopng",
                    label: "Convert all .tex to PNG",
                    // Keep the console open so the per-file summary can be read.
                    command: "\"{exe}\" --pause always decode --format png \"%1\"",
                },
                SubVerb {
                    key: "alltodds",
                    label: "Convert all .tex to DDS",
                    command: "\"{exe}\" --pause always decode --format dds \"%1\"",
                },
            ],
        },
    ];

    /// Registry path (under HKCU) of a menu's parent `shell\ltktexutils` node.
    fn menu_path(menu: &Menu) -> String {
        format!(
            "Software\\Classes\\{}\\shell\\{MENU_KEY}",
            menu.root.class()
        )
    }

    /// Registry path (under HKCU) of a sub-verb's node inside the cascading submenu.
    fn subverb_path(menu: &Menu, sub: &SubVerb) -> String {
        format!("{}\\shell\\{}", menu_path(menu), sub.key)
    }

    fn current_exe_string() -> Result<String> {
        let exe = std::env::current_exe()
            .wrap_err("failed to resolve the ltk-tex-utils executable path")?;
        Ok(exe.to_string_lossy().into_owned())
    }

    pub fn install(classic: bool) -> Result<()> {
        let exe = current_exe_string()?;
        let exe_dir = std::path::Path::new(&exe)
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();

        // Prefer the packaged menu: on Windows 11 it renders in BOTH the
        // modern menu and "Show more options", so installing the registry
        // cascade alongside it would only produce duplicates - including a
        // permanently empty flyout at the top of the modern menu, which can
        // render a `SubCommands` parent but never enumerates its nested
        // `shell` subkey. The registry verbs below remain as the fallback for
        // pre-Win11, a missing handler DLL, or Developer Mode being off.
        //
        // `--classic` opts out of the packaged menu entirely: the packaged
        // command has no placement control, while the registry cascade can
        // pin itself to the top of the .tex menu (`Position=Top`) - a
        // trade-off only the user can weigh.
        if classic {
            println!("Installing the classic registry menu (--classic).\n");
        } else {
            match super::modern::install(&exe_dir) {
                Ok(None) => {
                    if remove_classic_menus()? > 0 {
                        println!(
                            "Removed the classic registry menu (superseded by the packaged one)."
                        );
                    }
                    println!(
                        "{} Windows 11 context menu registered {}",
                        "\u{2713}".green().bold(),
                        "(restart Explorer if it doesn't show up right away)".dimmed()
                    );
                    println!(
                        "Right-click a .tex / .dds / .png file or a folder and open the \
                         '{MENU_LABEL}' menu."
                    );
                    println!("(pointing at {exe})");
                    println!(
                        "{}",
                        "Prefer the menu pinned to the top for .tex files? Re-run with \
                         'shell install --classic' (renders only under 'Show more options')."
                            .dimmed()
                    );
                    return Ok(());
                }
                Ok(Some(skip)) => println!(
                    "{} Windows 11 modern context menu skipped: {skip}\n  \
                     (falling back to the classic menu under 'Show more options')\n",
                    "note:".yellow().bold()
                ),
                Err(e) => println!(
                    "{} Windows 11 modern context menu registration failed:\n  {e:#}\n  \
                     (falling back to the classic menu under 'Show more options')\n",
                    "warning:".yellow().bold()
                ),
            }
        }

        // A package from an earlier install would duplicate (or shadow) the
        // registry verbs we are about to write - drop it.
        match super::modern::uninstall() {
            Ok(true) => println!("Removed a stale Windows 11 menu package registration."),
            Ok(false) => {}
            Err(e) => println!(
                "{} failed to remove the stale Windows 11 menu package:\n  {e:#}",
                "warning:".yellow().bold()
            ),
        }

        // First icon resource of the executable (embedded by the build script).
        let icon = format!("\"{exe}\",0");
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        for menu in MENUS {
            let path = menu_path(menu);
            // Clear any previous submenu subtree first so stale sub-verbs from an older
            // install don't linger alongside the new ones.
            match hkcu.delete_subkey_all(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(e).wrap_err_with(|| format!("failed to reset registry key {path}"));
                }
            }
            let (key, _) = hkcu
                .create_subkey(&path)
                .wrap_err_with(|| format!("failed to create registry key {path}"))?;
            // Parent cascading verb.
            key.set_value("MUIVerb", &MENU_LABEL)?;
            key.set_value("Icon", &icon)?;
            if menu.position_top {
                key.set_value("Position", &"Top")?;
            }
            // An (empty) SubCommands value tells Explorer to build a cascading menu from the
            // nested `shell` subkey rather than treating this as an invokable verb.
            key.set_value("SubCommands", &"")?;

            for sub in menu.subverbs {
                let sub_path = subverb_path(menu, sub);
                let (sub_key, _) = hkcu
                    .create_subkey(&sub_path)
                    .wrap_err_with(|| format!("failed to create registry key {sub_path}"))?;
                sub_key.set_value("", &sub.label)?;
                sub_key.set_value("Icon", &icon)?;
                // Lift Explorer's default 15-item cap for multi-selection; each selected
                // item still gets its own invocation.
                sub_key.set_value("MultiSelectModel", &"Player")?;

                let (command_key, _) = hkcu
                    .create_subkey(format!("{sub_path}\\command"))
                    .wrap_err_with(|| {
                        format!("failed to create registry key {sub_path}\\command")
                    })?;
                command_key.set_value("", &sub.command.replace("{exe}", &exe))?;

                tracing::info!(
                    "registered '{}' for {} -> {}",
                    sub.label,
                    menu.root.describe(),
                    sub_path
                );
            }
        }

        println!("ltk-tex-utils Explorer integration installed.");
        println!(
            "Right-click a .tex / .dds / .png file or a folder and open the '{MENU_LABEL}' menu."
        );
        println!("(pointing at {exe})");
        Ok(())
    }

    /// Delete every classic cascade from the registry; returns how many menus
    /// existed.
    fn remove_classic_menus() -> Result<usize> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let mut removed = 0usize;

        for menu in MENUS {
            let path = menu_path(menu);
            match hkcu.delete_subkey_all(&path) {
                Ok(()) => {
                    removed += 1;
                    tracing::info!("removed ltk-tex-utils menu ({path})");
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(e)
                        .wrap_err_with(|| format!("failed to delete registry key {path}"));
                }
            }
        }
        Ok(removed)
    }

    pub fn uninstall() -> Result<()> {
        let removed = remove_classic_menus()?;

        let modern_removed = match super::modern::uninstall() {
            Ok(removed) => removed,
            Err(e) => {
                println!(
                    "{} failed to remove the Windows 11 modern context menu:\n  {e:#}",
                    "warning:".yellow().bold()
                );
                false
            }
        };
        if modern_removed {
            println!("Windows 11 modern context menu removed.");
        }

        if removed == 0 && !modern_removed {
            println!("ltk-tex-utils Explorer integration was not installed; nothing to remove.");
        } else if removed > 0 {
            println!(
                "ltk-tex-utils Explorer integration removed ({removed} menu{}).",
                if removed == 1 { "" } else { "s" }
            );
        }
        Ok(())
    }

    pub fn status() -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let current = current_exe_string().ok();
        let exe_dir = current
            .as_deref()
            .and_then(|exe| std::path::Path::new(exe).parent())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();

        println!("{}", "ltk-tex-utils Explorer integration".bold());
        if let Some(exe) = &current {
            println!("  {} {}", "pointing at".dimmed(), exe.as_str().dimmed());
        }
        println!();

        let modern_version = super::modern::registered_version().unwrap_or_default();
        if let Err(e) = super::modern::print_status(&exe_dir, modern_version.as_deref()) {
            println!(
                "  {} modern menu (Win11)  status unavailable: {e:#}",
                "!".yellow().bold()
            );
        }

        // With the packaged menu registered, absent registry verbs are the
        // intended state, not eleven missing rows.
        let classic_any = MENUS.iter().any(|menu| {
            menu.subverbs.iter().any(|sub| {
                hkcu.open_subkey(format!("{}\\command", subverb_path(menu, sub)))
                    .is_ok()
            })
        });
        if !classic_any && modern_version.is_some() {
            println!(
                "    classic menu         {}",
                "not installed (the modern menu covers both menus)".dimmed()
            );
            return Ok(());
        }
        println!();

        // Column widths so the extension and label columns line up.
        let root_w = MENUS
            .iter()
            .map(|m| m.root.describe().len())
            .max()
            .unwrap_or(0);
        let label_w = MENUS
            .iter()
            .flat_map(|m| m.subverbs.iter().map(|s| s.label.len()))
            .max()
            .unwrap_or(0);

        let (mut installed, mut stale, mut missing) = (0usize, 0usize, 0usize);

        for menu in MENUS {
            let root = menu.root.describe();
            for sub in menu.subverbs {
                let sub_path = subverb_path(menu, sub);
                match hkcu.open_subkey(format!("{sub_path}\\command")) {
                    Ok(command_key) => {
                        let command: String = command_key.get_value("").unwrap_or_default();
                        let is_stale =
                            matches!(&current, Some(exe) if !command.contains(exe.as_str()));
                        if is_stale {
                            stale += 1;
                            // The path differs from ours, so show the full command it points at.
                            println!(
                                "  {} {:<root_w$}  {:<label_w$}  {}",
                                "!".yellow().bold(),
                                root,
                                sub.label,
                                command.yellow(),
                            );
                        } else {
                            installed += 1;
                            // Drop the redundant exe prefix; it's printed once above.
                            let args = current
                                .as_deref()
                                .map(|exe| command.replace(&format!("\"{exe}\" "), ""))
                                .unwrap_or_else(|| command.clone());
                            println!(
                                "  {} {:<root_w$}  {:<label_w$}  {}",
                                "\u{2713}".green().bold(),
                                root,
                                sub.label,
                                args.dimmed(),
                            );
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        missing += 1;
                        println!(
                            "  {} {:<root_w$}  {}",
                            "\u{2717}".red().bold(),
                            root,
                            sub.label.dimmed(),
                        );
                    }
                    Err(e) => {
                        return Err(e).wrap_err_with(|| {
                            format!("failed to read registry key {sub_path}\\command")
                        });
                    }
                }
            }
        }

        println!();
        let mut parts = Vec::new();
        if installed > 0 {
            parts.push(format!("{installed} installed").green().to_string());
        }
        if stale > 0 {
            parts.push(format!("{stale} stale").yellow().to_string());
        }
        if missing > 0 {
            parts.push(format!("{missing} missing").red().to_string());
        }
        if parts.is_empty() {
            parts.push("not installed".dimmed().to_string());
        }
        println!("  {}", parts.join(", "));
        Ok(())
    }
}

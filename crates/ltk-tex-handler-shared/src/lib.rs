//! Constants shared between the `.tex` shell handler DLL (`ltk-tex-thumb-handler`)
//! and the `ltk-tex-utils` CLI that installs and inspects it.
//!
//! These are the registry identifiers and the override toggle that both crates
//! must agree on: the DLL writes them during registration, and the CLI reads them
//! back to report status and to switch the DLL into override mode. Kept
//! dependency-free (plain `&str`, no `windows`/COM) so a cdylib and a bin can
//! share one source of truth without either pulling in the other's baggage.
//!
//! The GUID string forms live here; the DLL additionally holds the same GUIDs in
//! [`windows::core::GUID`] form (there is no const string→GUID conversion), so
//! those two representations must be kept in step within that crate.

/// CLSID of the TEX thumbnail handler COM server.
pub const CLSID_TEX_THUMB_HANDLER: &str = "{2f7e3e47-3b6b-4d59-9d42-4f4b0a5ba1b9}";
/// CLSID of the TEX preview handler COM server.
pub const CLSID_TEX_PREVIEW_HANDLER: &str = "{b1e4f2a8-7c3d-4e6f-9a1b-5d8c2f7e0a34}";
/// CLSID of the TEX property handler COM server.
pub const CLSID_TEX_PROPERTY_HANDLER: &str = "{c2f5a3b9-8d4e-4a6f-b1c7-3e9d0f2a5b48}";
/// CLSID of the `IExplorerCommand` context-menu server (Windows 11 modern menu).
/// Activated via packaged COM (the sparse package manifest), never via regsvr32.
pub const CLSID_TEX_EXPLORER_COMMAND: &str = "{6f8e2b34-9d1c-4a57-b8e0-2c3d4f5a6b71}";

/// IID_IThumbnailProvider - Explorer's thumbnail ShellEx slot.
pub const IID_ITHUMBNAILPROVIDER: &str = "{e357fccd-a995-4576-b01f-234630154e96}";
/// IID_IPreviewHandler - Explorer's preview ShellEx slot.
pub const IID_IPREVIEWHANDLER: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";

/// ProgID we claim as the `.tex` default when no other application owns the
/// type, so Explorer's Type column shows [`PROGID_TEX_FRIENDLY_NAME`] instead
/// of a description scavenged from some OpenWith entry (e.g. VS Code's "LaTeX
/// Source File"). Never claimed over a foreign owner, and released on
/// unregister only if still ours.
pub const PROGID_TEX: &str = "LeagueToolkit.Tex";
/// Friendly type name Explorer displays for [`PROGID_TEX`].
pub const PROGID_TEX_FRIENDLY_NAME: &str = "LoL Texture File";

/// HKLM key where override mode backs up the association it takes over, so that
/// unregistering can restore the previous owner.
pub const OVERRIDE_BACKUP_KEY: &str =
    "SOFTWARE\\LeagueToolkit\\ltk-tex-thumb-handler\\OverrideBackup";

/// Subkey of [`OVERRIDE_BACKUP_KEY`] recording the foreign `.tex\OpenWithProgids`
/// entries install removed (one subkey per source hive: `HKCU` / `HKLM`);
/// restored on unregister.
pub const OVERRIDE_BACKUP_OPENWITH_SUBKEY: &str = "OpenWithProgids";

/// Environment variable the DLL's `DllRegisterServer` reads to enable override
/// mode. Set by the CLI (`handler install`, unless `--no-override`) and the
/// install script before invoking `regsvr32`, because COM registration
/// entrypoints take no args.
pub const OVERRIDE_ENV: &str = "LTK_TEX_HANDLER_OVERRIDE";

/// File name of the handler DLL, shared by the CLI (which locates it) and the
/// sparse package manifest (which references it relative to the install dir).
pub const HANDLER_DLL_FILE_NAME: &str = "ltk_tex_thumb_handler.dll";
/// File name of the CLI executable, referenced by the sparse package manifest.
pub const CLI_EXE_FILE_NAME: &str = "ltk-tex-utils.exe";

/// User-visible title of the cascading Explorer context menu, shared by the
/// classic registry cascade (`MUIVerb`), the packaged Windows 11 command
/// (`IExplorerCommand::GetTitle`), and the package's display names so every
/// surface reads the same.
pub const MENU_LABEL: &str = "LTK Toolz";

/// Identity `Name` of the sparse package that puts the ltk-tex-utils commands
/// into the Windows 11 modern context menu.
pub const PACKAGE_IDENTITY_NAME: &str = "LeagueToolkit.ltk-tex-utils";
/// Identity `Publisher` of the sparse package. Plain CN: the package is
/// registered unsigned through Developer Mode's loose-manifest path, which
/// rejects the `OID.2.25...` unsigned-namespace marker used by `-AllowUnsigned`.
pub const PACKAGE_PUBLISHER: &str = "CN=LeagueToolkit";

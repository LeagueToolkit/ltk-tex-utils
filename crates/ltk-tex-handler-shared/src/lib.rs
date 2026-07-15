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

/// IID_IThumbnailProvider - Explorer's thumbnail ShellEx slot.
pub const IID_ITHUMBNAILPROVIDER: &str = "{e357fccd-a995-4576-b01f-234630154e96}";
/// IID_IPreviewHandler - Explorer's preview ShellEx slot.
pub const IID_IPREVIEWHANDLER: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";

/// HKLM key where override mode backs up the association it takes over, so that
/// unregistering can restore the previous owner.
pub const OVERRIDE_BACKUP_KEY: &str =
    "SOFTWARE\\LeagueToolkit\\ltk-tex-thumb-handler\\OverrideBackup";

/// Environment variable the DLL's `DllRegisterServer` reads to enable override
/// mode. Set by the CLI (`handler install --override`) and the install script
/// before invoking `regsvr32`, because COM registration entrypoints take no args.
pub const OVERRIDE_ENV: &str = "LTK_TEX_HANDLER_OVERRIDE";

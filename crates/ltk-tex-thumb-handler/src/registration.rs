// =============================================================================
// REGISTRATION HELPERS (following Microsoft registry patterns)
// =============================================================================

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::Win32::Foundation::{HMODULE, *};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::UI::Shell::PropertiesSystem::{
    PSRegisterPropertySchema, PSUnregisterPropertySchema,
};
use windows::core::*;
use winreg::HKEY;
use winreg::RegKey;
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_WRITE};

pub const SZ_CLSID_TEXTHUMBHANDLER: &str = "{2f7e3e47-3b6b-4d59-9d42-4f4b0a5ba1b9}";
pub const SZ_TEXTHUMBHANDLER: &str = "LTK TEX Thumbnail Handler";

pub const SZ_CLSID_TEXPREVIEWHANDLER: &str = "{b1e4f2a8-7c3d-4e6f-9a1b-5d8c2f7e0a34}";
pub const SZ_TEXPREVIEWHANDLER: &str = "LTK TEX Preview Handler";

/// IID_IPreviewHandler — the ShellEx key under which preview handlers register.
const SZ_PREVIEWHANDLER_IID: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";
/// Well-known AppID for the 64-bit prevhost.exe surrogate (system32\prevhost.exe)
/// that hosts preview handlers. Must match the DLL's bitness — the 32-bit
/// surrogate ({534A1E02-...}, SysWOW64) can't load a 64-bit handler.
const SZ_PREVHOST_APPID: &str = "{6d2b5079-2f0b-48dd-ab7f-97cec514d30b}";
/// Registry list Explorer consults to enumerate installed preview handlers.
const SZ_PREVIEWHANDLERS_KEY: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PreviewHandlers";

pub const SZ_CLSID_TEXPROPERTYHANDLER: &str = "{c2f5a3b9-8d4e-4a6f-b1c7-3e9d0f2a5b48}";
pub const SZ_TEXPROPERTYHANDLER: &str = "LTK TEX Property Handler";
/// Per-extension property handler registration lives under this HKLM key.
const SZ_PROPERTYHANDLERS_KEY: &str =
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PropertySystem\\PropertyHandlers";
/// Custom property schema, registered so our LeagueToolkit.Tex.* props get labels.
const PROPDESC_XML: &str = include_str!("../ltk_tex.propdesc");
const PROPDESC_FILENAME: &str = "ltk_tex.propdesc";

pub struct RegistryEntry {
    pub hkeyRoot: HKEY,
    pub pszKeyName: String,
    pub pszValueName: Option<String>,
    pub pszData: String,
}

impl RegistryEntry {
    pub fn new(
        root: HKEY,
        key: impl Into<String>,
        value: Option<impl Into<String>>,
        data: impl Into<String>,
    ) -> Self {
        Self {
            hkeyRoot: root,
            pszKeyName: key.into(),
            pszValueName: value.map(|v| v.into()),
            pszData: data.into(),
        }
    }
}

/// The .propdesc schema file lives next to the DLL.
fn propdesc_path(dll_path: &str) -> PathBuf {
    let mut p = PathBuf::from(dll_path);
    p.set_file_name(PROPDESC_FILENAME);
    p
}

/// Resolve this DLL's own path (used by unregister, which has no fn pointer).
fn this_dll_path() -> Option<String> {
    let mut hmodule = HMODULE(std::ptr::null_mut());
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            PCWSTR(this_dll_path as *const u16),
            &mut hmodule,
        )
        .ok()?;
    }
    let mut buf = [0u16; 32768];
    let len = unsafe { GetModuleFileNameW(hmodule, &mut buf) };
    if len == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

fn wide(path: &std::path::Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Write the embedded schema next to the DLL and register it (best effort).
unsafe fn register_schema(dll_path: &str) {
    let path = propdesc_path(dll_path);
    if std::fs::write(&path, PROPDESC_XML).is_err() {
        return;
    }
    let _ = unsafe { PSRegisterPropertySchema(PCWSTR(wide(&path).as_ptr())) };
}

/// Unregister the schema and remove the file (best effort).
unsafe fn unregister_schema() {
    if let Some(dll) = this_dll_path() {
        let path = propdesc_path(&dll);
        let _ = unsafe { PSUnregisterPropertySchema(PCWSTR(wide(&path).as_ptr())) };
        let _ = std::fs::remove_file(&path);
    }
}

/// Creates a registry key and sets its value (following Microsoft's CreateRegKeyAndSetValue pattern)
fn create_reg_key_and_set_value(entry: &RegistryEntry) -> Result<()> {
    let root = RegKey::predef(entry.hkeyRoot);
    let (key, _) = root
        .create_subkey(&entry.pszKeyName)
        .map_err(|_| Error::from(E_FAIL))?;

    let value_name = entry.pszValueName.as_deref().unwrap_or("");
    key.set_value(value_name, &entry.pszData)
        .map_err(|_| Error::from(E_FAIL))?;

    Ok(())
}

/// Register COM server and shell extension (following Microsoft pattern)
pub unsafe fn register_server(dll_register_server_fn: *const u16) -> Result<()> {
    // Get DLL path
    let mut hmodule = HMODULE(std::ptr::null_mut());
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            PCWSTR(dll_register_server_fn),
            &mut hmodule,
        )?
    };

    let mut buf = [0u16; 32768];
    let len = unsafe { GetModuleFileNameW(hmodule, &mut buf) };
    if len == 0 {
        return Err(Error::from(E_FAIL));
    }
    let dll_path = String::from_utf16_lossy(&buf[..len as usize]);

    let entries = vec![
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}", SZ_CLSID_TEXTHUMBHANDLER),
            None::<String>,
            SZ_TEXTHUMBHANDLER,
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}\\InprocServer32", SZ_CLSID_TEXTHUMBHANDLER),
            None::<String>,
            &dll_path,
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}\\InprocServer32", SZ_CLSID_TEXTHUMBHANDLER),
            Some("ThreadingModel"),
            "Apartment",
        ),
        // .tex file association with IThumbnailProvider
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            ".tex\\ShellEx\\{e357fccd-a995-4576-b01f-234630154e96}",
            None::<String>,
            SZ_CLSID_TEXTHUMBHANDLER,
        ),
        // ---- Preview handler (Explorer preview pane / Alt+P) ----
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}", SZ_CLSID_TEXPREVIEWHANDLER),
            None::<String>,
            SZ_TEXPREVIEWHANDLER,
        ),
        // Host the handler out-of-process in prevhost.exe (isolation + stability)
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}", SZ_CLSID_TEXPREVIEWHANDLER),
            Some("AppID"),
            SZ_PREVHOST_APPID,
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}\\InprocServer32", SZ_CLSID_TEXPREVIEWHANDLER),
            None::<String>,
            &dll_path,
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}\\InprocServer32", SZ_CLSID_TEXPREVIEWHANDLER),
            Some("ThreadingModel"),
            "Apartment",
        ),
        // .tex association with IPreviewHandler
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!(".tex\\ShellEx\\{}", SZ_PREVIEWHANDLER_IID),
            None::<String>,
            SZ_CLSID_TEXPREVIEWHANDLER,
        ),
        // ---- Property handler (Details pane, columns, tooltips, search) ----
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}", SZ_CLSID_TEXPROPERTYHANDLER),
            None::<String>,
            SZ_TEXPROPERTYHANDLER,
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}\\InprocServer32", SZ_CLSID_TEXPROPERTYHANDLER),
            None::<String>,
            &dll_path,
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            format!("CLSID\\{}\\InprocServer32", SZ_CLSID_TEXPROPERTYHANDLER),
            Some("ThreadingModel"),
            "Both",
        ),
        // Which properties Explorer shows for .tex (Details pane / tooltip).
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            "SystemFileAssociations\\.tex",
            Some("FullDetails"),
            "prop:System.PropGroup.Image;System.Image.Dimensions;\
             LeagueToolkit.Tex.Format;LeagueToolkit.Tex.MipLevels;LeagueToolkit.Tex.HasAlpha",
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            "SystemFileAssociations\\.tex",
            Some("PreviewDetails"),
            "prop:System.Image.Dimensions;LeagueToolkit.Tex.Format;\
             LeagueToolkit.Tex.MipLevels;LeagueToolkit.Tex.HasAlpha",
        ),
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            "SystemFileAssociations\\.tex",
            Some("InfoTip"),
            "prop:System.Image.Dimensions;LeagueToolkit.Tex.Format;LeagueToolkit.Tex.MipLevels",
        ),
    ];

    // Register all entries
    for entry in &entries {
        create_reg_key_and_set_value(entry)?;
    }

    // Approve shell extension (Windows requirement)
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (approved, _) = hklm
        .create_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved")
        .map_err(|_| Error::from(E_FAIL))?;

    approved
        .set_value(SZ_CLSID_TEXTHUMBHANDLER, &SZ_TEXTHUMBHANDLER)
        .map_err(|_| Error::from(E_FAIL))?;
    approved
        .set_value(SZ_CLSID_TEXPREVIEWHANDLER, &SZ_TEXPREVIEWHANDLER)
        .map_err(|_| Error::from(E_FAIL))?;

    // Add the preview handler to the shell's PreviewHandlers list.
    let (preview_handlers, _) = hklm
        .create_subkey(SZ_PREVIEWHANDLERS_KEY)
        .map_err(|_| Error::from(E_FAIL))?;
    preview_handlers
        .set_value(SZ_CLSID_TEXPREVIEWHANDLER, &SZ_TEXPREVIEWHANDLER)
        .map_err(|_| Error::from(E_FAIL))?;

    // Associate the property handler with .tex (HKLM PropertyHandlers list).
    let (prop_handler, _) = hklm
        .create_subkey(format!("{}\\.tex", SZ_PROPERTYHANDLERS_KEY))
        .map_err(|_| Error::from(E_FAIL))?;
    prop_handler
        .set_value("", &SZ_CLSID_TEXPROPERTYHANDLER)
        .map_err(|_| Error::from(E_FAIL))?;

    // Register the custom property schema (best effort — canonical props still
    // work without it; only our labelled TEX Format / Mip / Alpha rows need it).
    unsafe { register_schema(&dll_path) };

    // Disable process isolation for better debugging
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    if let Ok(clsid_key) =
        hkcr.open_subkey_with_flags(format!("CLSID\\{}", SZ_CLSID_TEXTHUMBHANDLER), KEY_WRITE)
    {
        let _ = clsid_key.set_value("DisableProcessIsolation", &1u32);
    }

    // Notify shell of changes (following Microsoft pattern)
    use windows::Win32::UI::Shell::SHChangeNotify;
    use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };

    Ok(())
}

/// Unregister COM server and shell extension (following Microsoft pattern)
pub unsafe fn unregister_server() -> Result<()> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);

    let _ = hkcr.delete_subkey_all(format!("CLSID\\{}", SZ_CLSID_TEXTHUMBHANDLER));
    let _ = hkcr.delete_subkey_all(".tex\\ShellEx\\{e357fccd-a995-4576-b01f-234630154e96}");

    // Preview handler
    let _ = hkcr.delete_subkey_all(format!("CLSID\\{}", SZ_CLSID_TEXPREVIEWHANDLER));
    let _ = hkcr.delete_subkey_all(format!(".tex\\ShellEx\\{}", SZ_PREVIEWHANDLER_IID));

    // Remove from approved extensions (needs write access to delete values)
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(approved) = hklm.open_subkey_with_flags(
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved",
        KEY_WRITE,
    ) {
        let _ = approved.delete_value(SZ_CLSID_TEXTHUMBHANDLER);
        let _ = approved.delete_value(SZ_CLSID_TEXPREVIEWHANDLER);
    }

    // Remove from the PreviewHandlers list
    if let Ok(preview_handlers) = hklm.open_subkey_with_flags(SZ_PREVIEWHANDLERS_KEY, KEY_WRITE) {
        let _ = preview_handlers.delete_value(SZ_CLSID_TEXPREVIEWHANDLER);
    }

    // Property handler
    let _ = hkcr.delete_subkey_all(format!("CLSID\\{}", SZ_CLSID_TEXPROPERTYHANDLER));
    let _ = hklm.delete_subkey_all(format!("{}\\.tex", SZ_PROPERTYHANDLERS_KEY));
    if let Ok(assoc) = hkcr.open_subkey_with_flags("SystemFileAssociations\\.tex", KEY_WRITE) {
        let _ = assoc.delete_value("FullDetails");
        let _ = assoc.delete_value("PreviewDetails");
        let _ = assoc.delete_value("InfoTip");
    }
    unsafe { unregister_schema() };

    Ok(())
}

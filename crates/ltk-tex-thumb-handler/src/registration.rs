// =============================================================================
// REGISTRATION HELPERS (following Microsoft registry patterns)
// =============================================================================

use windows::Win32::Foundation::{HMODULE, *};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GetModuleFileNameW, GetModuleHandleExW,
};
use windows::core::*;
use winreg::HKEY;
use winreg::RegKey;
use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_WRITE};

pub const SZ_CLSID_TEXTHUMBHANDLER: &str = "{2f7e3e47-3b6b-4d59-9d42-4f4b0a5ba1b9}";
pub const SZ_TEXTHUMBHANDLER: &str = "LTK TEX Thumbnail Handler";

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
        // .dds file association with IThumbnailProvider
        RegistryEntry::new(
            HKEY_CLASSES_ROOT,
            ".dds\\ShellEx\\{e357fccd-a995-4576-b01f-234630154e96}",
            None::<String>,
            SZ_CLSID_TEXTHUMBHANDLER,
        ),
    ];

    for (i, entry) in entries.iter().enumerate() {
        if let Err(e) = create_reg_key_and_set_value(entry) {
            // If this is the .dds registration (last entry), log but don't fail
            if i == entries.len() - 1 && entry.pszKeyName.contains(".dds") {
                continue;
            }

            return Err(e);
        }
    }

    // Approve shell extension (Windows requirement)
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let (approved, _) = hklm
        .create_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved")
        .map_err(|_| Error::from(E_FAIL))?;

    approved
        .set_value(SZ_CLSID_TEXTHUMBHANDLER, &SZ_TEXTHUMBHANDLER)
        .map_err(|_| Error::from(E_FAIL))?;

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
    let _ = hkcr.delete_subkey_all(".dds\\ShellEx\\{e357fccd-a995-4576-b01f-234630154e96}");

    // Remove from approved extensions
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(approved) =
        hklm.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Shell Extensions\\Approved")
    {
        let _ = approved.delete_value(SZ_CLSID_TEXTHUMBHANDLER);
    }

    Ok(())
}

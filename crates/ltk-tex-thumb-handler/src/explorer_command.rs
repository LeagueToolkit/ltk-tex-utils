// =============================================================================
// EXPLORER COMMAND (Windows 11 modern context menu)
// =============================================================================
//
// `IExplorerCommand` implementation behind the cascading "LTK Toolz" entry
// in the Windows 11 modern context menu. The classic registry verbs written by
// `ltk-tex-utils shell install` only render in the legacy menu ("Show more
// options"); the modern menu builds its top level exclusively from
// IExplorerCommand handlers of packaged apps, so the CLI registers a sparse
// package whose manifest maps `.tex`/`.dds`/`.png`/`Directory` to
// [`crate::CLSID_TEX_EXPLORER_COMMAND`] and points packaged COM at this DLL.
// That is also why this class is absent from `registration.rs`: activation
// goes through the package manifest, not HKCR.
//
// One parent command (ECF_HASSUBCOMMANDS) exposes every convert verb; each
// sub-verb hides itself (ECS_HIDDEN) when nothing in the selection applies to
// it. Invoke mirrors the classic verbs: one CLI process per selected item,
// using the exact same argument lines.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::UI::Shell::{
    ECF_DEFAULT, ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN, IEnumExplorerCommand,
    IEnumExplorerCommand_Impl, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray, SHStrDupW,
    SIGDN_FILESYSPATH,
};
use windows::core::*;

use ltk_tex_handler_shared::{CLI_EXE_FILE_NAME, MENU_LABEL};

/// One convert verb inside the cascading menu. Labels and CLI argument lines
/// are kept in lockstep with the registry verbs in the CLI's `shell` module.
#[derive(Copy, Clone)]
enum SubVerb {
    TexToPng,
    TexToDds,
    ToTex,
    DirAllToPng,
    DirAllToDds,
}

/// Every sub-verb, in menu order.
const SUB_VERBS: &[SubVerb] = &[
    SubVerb::TexToPng,
    SubVerb::TexToDds,
    SubVerb::ToTex,
    SubVerb::DirAllToPng,
    SubVerb::DirAllToDds,
];

impl SubVerb {
    fn label(self) -> &'static str {
        match self {
            SubVerb::TexToPng => "Convert to PNG",
            SubVerb::TexToDds => "Convert to DDS",
            SubVerb::ToTex => "Convert to TEX",
            SubVerb::DirAllToPng => "Convert all .tex to PNG",
            SubVerb::DirAllToDds => "Convert all .tex to DDS",
        }
    }

    /// Whether this verb can act on the given selected item.
    fn applies_to(self, path: &Path) -> bool {
        let ext_is = |wanted: &[&str]| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| wanted.iter().any(|w| e.eq_ignore_ascii_case(w)))
        };
        match self {
            SubVerb::TexToPng | SubVerb::TexToDds => ext_is(&["tex"]),
            SubVerb::ToTex => ext_is(&["dds", "png"]),
            SubVerb::DirAllToPng | SubVerb::DirAllToDds => path.is_dir(),
        }
    }

    /// CLI arguments for one selected item (same lines as the registry verbs;
    /// folders keep the console open so the per-file summary can be read).
    fn args(self, path: &Path) -> Vec<std::ffi::OsString> {
        let mut args: Vec<std::ffi::OsString> = match self {
            SubVerb::TexToPng => ["--pause", "on-error", "decode", "--format", "png"],
            SubVerb::TexToDds => ["--pause", "on-error", "decode", "--format", "dds"],
            SubVerb::ToTex => {
                return vec![
                    "--pause".into(),
                    "on-error".into(),
                    "encode".into(),
                    path.into(),
                ];
            }
            SubVerb::DirAllToPng => ["--pause", "always", "decode", "--format", "png"],
            SubVerb::DirAllToDds => ["--pause", "always", "decode", "--format", "dds"],
        }
        .iter()
        .map(Into::into)
        .collect();
        args.push(path.into());
        args
    }
}

/// Directory this DLL was loaded from (the sparse package's external location,
/// where the CLI executable lives too).
fn dll_dir() -> Option<PathBuf> {
    let hinst = unsafe { *std::ptr::addr_of!(crate::G_HINST) };
    let mut buf = [0u16; 32768];
    let len = unsafe { GetModuleFileNameW(HMODULE(hinst.0), &mut buf) } as usize;
    if len == 0 || len >= buf.len() {
        return None;
    }
    PathBuf::from(String::from_utf16_lossy(&buf[..len]))
        .parent()
        .map(Path::to_path_buf)
}

fn cli_exe() -> Option<PathBuf> {
    let exe = dll_dir()?.join(CLI_EXE_FILE_NAME);
    exe.is_file().then_some(exe)
}

/// Copy a Rust string into a COM-allocated `PWSTR` (callers free it).
fn com_string(s: &str) -> Result<PWSTR> {
    let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { SHStrDupW(PCWSTR(wide.as_ptr())) }
}

/// `"<exe path>,0"` — first icon resource of the CLI executable.
fn icon_location() -> Result<PWSTR> {
    let exe = cli_exe().ok_or_else(|| Error::from(E_NOTIMPL))?;
    com_string(&format!("{},0", exe.display()))
}

/// Filesystem paths of the current selection (items without one are skipped).
fn selection_paths(items: Option<&IShellItemArray>) -> Vec<PathBuf> {
    let Some(items) = items else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    unsafe {
        let count = items.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(item) = items.GetItemAt(i) else {
                continue;
            };
            if let Ok(name) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                if let Ok(s) = name.to_string() {
                    paths.push(PathBuf::from(s));
                }
                CoTaskMemFree(Some(name.0 as *const c_void));
            }
        }
    }
    paths
}

// =============================================================================
// PARENT COMMAND (the cascading "ltk-tex-utils" entry)
// =============================================================================

#[implement(IExplorerCommand)]
pub struct CTexContextMenu;

impl IExplorerCommand_Impl for CTexContextMenu_Impl {
    fn GetTitle(&self, _items: Option<&IShellItemArray>) -> Result<PWSTR> {
        com_string(MENU_LABEL)
    }

    fn GetIcon(&self, _items: Option<&IShellItemArray>) -> Result<PWSTR> {
        icon_location()
    }

    fn GetToolTip(&self, _items: Option<&IShellItemArray>) -> Result<PWSTR> {
        Err(Error::from(E_NOTIMPL))
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(crate::CLSID_TEX_EXPLORER_COMMAND)
    }

    fn GetState(&self, _items: Option<&IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        // The package manifest already scopes the menu to .tex/.dds/.png/Directory.
        Ok(ECS_ENABLED.0 as u32)
    }

    fn Invoke(&self, _items: Option<&IShellItemArray>, _pbc: Option<&IBindCtx>) -> Result<()> {
        // Never invoked: ECF_HASSUBCOMMANDS makes this a flyout, not a verb.
        Ok(())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_HASSUBCOMMANDS.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        let commands = SUB_VERBS
            .iter()
            .map(|&verb| CTexSubCommand { verb }.into())
            .collect();
        Ok(CEnumExplorerCommand::new(commands).into())
    }
}

pub fn CTexContextMenu_CreateInstance(riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    let unknown: IUnknown = CTexContextMenu.into();
    unsafe { unknown.query(riid, ppv) }
}

// =============================================================================
// SUB-COMMANDS (one per convert verb)
// =============================================================================

#[implement(IExplorerCommand)]
struct CTexSubCommand {
    verb: SubVerb,
}

impl IExplorerCommand_Impl for CTexSubCommand_Impl {
    fn GetTitle(&self, _items: Option<&IShellItemArray>) -> Result<PWSTR> {
        com_string(self.verb.label())
    }

    fn GetIcon(&self, _items: Option<&IShellItemArray>) -> Result<PWSTR> {
        icon_location()
    }

    fn GetToolTip(&self, _items: Option<&IShellItemArray>) -> Result<PWSTR> {
        Err(Error::from(E_NOTIMPL))
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(GUID::zeroed())
    }

    fn GetState(&self, items: Option<&IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        let visible = selection_paths(items)
            .iter()
            .any(|p| self.verb.applies_to(p));
        Ok(if visible { ECS_ENABLED.0 } else { ECS_HIDDEN.0 } as u32)
    }

    fn Invoke(&self, items: Option<&IShellItemArray>, _pbc: Option<&IBindCtx>) -> Result<()> {
        let exe = cli_exe().ok_or_else(|| Error::from(E_FAIL))?;
        // One process per item, matching the classic verbs' per-item invocation.
        for path in selection_paths(items) {
            if self.verb.applies_to(&path) {
                let _ = std::process::Command::new(&exe)
                    .args(self.verb.args(&path))
                    .spawn();
            }
        }
        Ok(())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(Error::from(E_NOTIMPL))
    }
}

// =============================================================================
// SUB-COMMAND ENUMERATOR
// =============================================================================

#[implement(IEnumExplorerCommand)]
struct CEnumExplorerCommand {
    commands: Vec<IExplorerCommand>,
    index: AtomicUsize,
}

impl CEnumExplorerCommand {
    fn new(commands: Vec<IExplorerCommand>) -> Self {
        Self {
            commands,
            index: AtomicUsize::new(0),
        }
    }
}

impl IEnumExplorerCommand_Impl for CEnumExplorerCommand_Impl {
    fn Next(
        &self,
        celt: u32,
        puicommand: *mut Option<IExplorerCommand>,
        pceltfetched: *mut u32,
    ) -> HRESULT {
        if puicommand.is_null() {
            return E_POINTER;
        }
        let mut fetched = 0usize;
        while fetched < celt as usize {
            let i = self.index.load(Ordering::Relaxed);
            if i >= self.commands.len() {
                break;
            }
            self.index.store(i + 1, Ordering::Relaxed);
            unsafe {
                *puicommand.add(fetched) = Some(self.commands[i].clone());
            }
            fetched += 1;
        }
        if !pceltfetched.is_null() {
            unsafe {
                *pceltfetched = fetched as u32;
            }
        }
        if fetched == celt as usize {
            S_OK
        } else {
            S_FALSE
        }
    }

    fn Skip(&self, celt: u32) -> Result<()> {
        self.index.fetch_add(celt as usize, Ordering::Relaxed);
        Ok(())
    }

    fn Reset(&self) -> Result<()> {
        self.index.store(0, Ordering::Relaxed);
        Ok(())
    }

    fn Clone(&self) -> Result<IEnumExplorerCommand> {
        let clone = CEnumExplorerCommand {
            commands: self.commands.clone(),
            index: AtomicUsize::new(self.index.load(Ordering::Relaxed)),
        };
        Ok(clone.into())
    }
}

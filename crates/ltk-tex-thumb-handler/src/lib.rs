// THIS CODE IS BASED ON MICROSOFT'S RECIPETHUMBNAILPROVIDER SAMPLE
// Adapted for LTK TEX file thumbnail generation
//
// Original Microsoft sample:
// https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/Win7Samples/winui/shell/appshellintegration/RecipeThumbnailProvider

#![cfg(windows)]
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use windows::Win32::Foundation::*;
use windows::core::*;

mod class_factory;
mod debug;
mod image_processing;
mod registration;
mod thumbnail_provider;

// Re-export for internal use
use class_factory::{C_RGCLASSOBJECTINIT, CClassFactory};
use registration::{register_server, unregister_server};

// =============================================================================
// CONSTANTS AND GLOBALS
// =============================================================================

/// CLSID for the TEX Thumbnail Handler
/// {2f7e3e47-3b6b-4d59-9d42-4f4b0a5ba1b9}
pub const CLSID_TEX_THUMB_HANDLER: GUID = GUID::from_u128(0x2f7e3e47_3b6b_4d59_9d42_4f4b0a5ba1b9);

/// Module reference count for DLL lifetime management
static G_CREF_MODULE: AtomicI32 = AtomicI32::new(0);

/// DLL module handle
#[allow(dead_code)]
static mut G_HINST: HINSTANCE = HINSTANCE(std::ptr::null_mut());

// =============================================================================
// MODULE REFERENCE COUNTING (following Microsoft pattern)
// =============================================================================

pub(crate) fn DllAddRef() {
    G_CREF_MODULE.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn DllRelease() {
    G_CREF_MODULE.fetch_sub(1, Ordering::SeqCst);
}

// =============================================================================
// STANDARD DLL EXPORTS (following Microsoft Dll.cpp pattern)
// =============================================================================

/// Standard DLL entry point
#[unsafe(no_mangle)]
unsafe extern "system" fn DllMain(
    hinstance: HINSTANCE,
    reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    if reason == 1 {
        // DLL_PROCESS_ATTACH
        unsafe {
            G_HINST = hinstance;
        }
    }
    TRUE
}

/// Returns S_OK if DLL can be unloaded, S_FALSE otherwise
///
/// # Safety
/// This function is safe to call. It only reads an atomic counter.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if G_CREF_MODULE.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

/// Creates a class factory for the requested CLSID
///
/// # Safety
/// The caller must ensure that `rclsid` and `riid` point to valid GUIDs,
/// and `ppv` points to a valid pointer location.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() {
        return E_INVALIDARG;
    }

    unsafe { CClassFactory::create_instance(&*rclsid, C_RGCLASSOBJECTINIT, riid, ppv) }
}

/// Registers this COM server (following Microsoft's DllRegisterServer pattern)
///
/// # Safety
/// This function modifies the Windows registry and requires administrative privileges.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    match unsafe { register_server(DllRegisterServer as *const u16) } {
        Ok(()) => S_OK,
        Err(e) => e.into(),
    }
}

/// Unregisters this COM server (following Microsoft's DllUnregisterServer pattern)
///
/// # Safety
/// This function modifies the Windows registry and requires administrative privileges.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    match unsafe { unregister_server() } {
        Ok(()) => S_OK,
        Err(e) => e.into(),
    }
}

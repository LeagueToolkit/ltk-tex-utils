// =============================================================================
// TEX PROPERTY HANDLER (Details pane, columns, tooltips, Windows Search)
//
// Exposes texture metadata as shell properties by decoding the .tex on
// Initialize and delegating IPropertyStore reads to an in-memory store.
// =============================================================================

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::AtomicI32;

use windows::Win32::Foundation::*;
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl, IPropertyStore, IPropertyStore_Impl,
    PROPERTYKEY, PSCreateMemoryPropertyStore,
};
use windows::core::*;

use crate::image_processing::{TexMeta, decode_tex_with_meta};
use crate::utils::read_stream_to_bytes;

// System.Image.* property set (built-in, labelled by the shell automatically).
const FMTID_IMAGE: GUID = GUID::from_u128(0x6444048F_4C8B_11D1_8B70_080036B11A03);
const PID_IMAGE_HORIZONTAL_SIZE: u32 = 3; // System.Image.HorizontalSize
const PID_IMAGE_VERTICAL_SIZE: u32 = 4; // System.Image.VerticalSize
const PID_IMAGE_DIMENSIONS: u32 = 13; // System.Image.Dimensions

// Our custom property set - the formatID/propIDs MUST match ltk_tex.propdesc.
const FMTID_LTK: GUID = GUID::from_u128(0x8F3E9A21_4B7C_4D2E_9C1A_6E5D4F3A2B10);
const PID_LTK_FORMAT: u32 = 2;
const PID_LTK_MIP_LEVELS: u32 = 3;
const PID_LTK_HAS_ALPHA: u32 = 4;

const fn pkey(fmtid: GUID, pid: u32) -> PROPERTYKEY {
    PROPERTYKEY { fmtid, pid }
}

#[implement(IInitializeWithStream, IPropertyStore)]
pub struct CTexPropertyHandler {
    #[allow(dead_code)]
    cRef: AtomicI32,
    store: Mutex<Option<IPropertyStore>>,
}

impl Default for CTexPropertyHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CTexPropertyHandler {
    pub fn new() -> Self {
        Self {
            cRef: AtomicI32::new(1),
            store: Mutex::new(None),
        }
    }
}

/// Decode metadata into a fresh in-memory property store.
fn build_store(meta: &TexMeta) -> Result<IPropertyStore> {
    let mut ppv: *mut c_void = std::ptr::null_mut();
    unsafe { PSCreateMemoryPropertyStore(&IPropertyStore::IID, &mut ppv) }?;
    let store: IPropertyStore = unsafe { IPropertyStore::from_raw(ppv) };

    let dims = PROPVARIANT::from(format!("{} x {}", meta.width, meta.height).as_str());
    unsafe {
        store.SetValue(
            &pkey(FMTID_IMAGE, PID_IMAGE_HORIZONTAL_SIZE),
            &PROPVARIANT::from(meta.width),
        )?;
        store.SetValue(
            &pkey(FMTID_IMAGE, PID_IMAGE_VERTICAL_SIZE),
            &PROPVARIANT::from(meta.height),
        )?;
        store.SetValue(&pkey(FMTID_IMAGE, PID_IMAGE_DIMENSIONS), &dims)?;
        store.SetValue(
            &pkey(FMTID_LTK, PID_LTK_FORMAT),
            &PROPVARIANT::from(meta.format),
        )?;
        store.SetValue(
            &pkey(FMTID_LTK, PID_LTK_MIP_LEVELS),
            &PROPVARIANT::from(meta.mip_count),
        )?;
        store.SetValue(
            &pkey(FMTID_LTK, PID_LTK_HAS_ALPHA),
            &PROPVARIANT::from(meta.has_alpha),
        )?;
    }
    Ok(store)
}

impl IInitializeWithStream_Impl for CTexPropertyHandler_Impl {
    fn Initialize(&self, pstream: Option<&IStream>, _grfmode: u32) -> Result<()> {
        let mut guard = self.store.lock().unwrap();
        if guard.is_some() {
            return Err(Error::from(E_UNEXPECTED));
        }
        let stream = pstream.ok_or(Error::from(E_INVALIDARG))?;
        let bytes = unsafe { read_stream_to_bytes(stream) }?;
        let (_, _, _, meta) = decode_tex_with_meta(&bytes)?;
        *guard = Some(build_store(&meta)?);
        Ok(())
    }
}

impl IPropertyStore_Impl for CTexPropertyHandler_Impl {
    fn GetCount(&self) -> Result<u32> {
        let guard = self.store.lock().unwrap();
        match guard.as_ref() {
            Some(s) => unsafe { s.GetCount() },
            None => Ok(0),
        }
    }

    fn GetAt(&self, iprop: u32, pkey: *mut PROPERTYKEY) -> Result<()> {
        let guard = self.store.lock().unwrap();
        match guard.as_ref() {
            Some(s) => unsafe { s.GetAt(iprop, pkey) },
            None => Err(Error::from(E_FAIL)),
        }
    }

    fn GetValue(&self, key: *const PROPERTYKEY) -> Result<PROPVARIANT> {
        let guard = self.store.lock().unwrap();
        match guard.as_ref() {
            Some(s) => unsafe { s.GetValue(key) },
            None => Err(Error::from(E_FAIL)),
        }
    }

    fn SetValue(&self, _key: *const PROPERTYKEY, _propvar: *const PROPVARIANT) -> Result<()> {
        // Computed, read-only metadata.
        Err(Error::from(E_ACCESSDENIED))
    }

    fn Commit(&self) -> Result<()> {
        Ok(())
    }
}

pub fn CTexPropertyHandler_CreateInstance(riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    let handler = CTexPropertyHandler::new();
    let unknown: IUnknown = handler.into();
    unsafe { unknown.query(riid, ppv) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
    use windows::Win32::UI::Shell::SHCreateMemStream;

    fn bgra8_tex(width: u16, height: u16, pixels_bgra: &[u8]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(b"TEX\0");
        f.extend_from_slice(&width.to_le_bytes());
        f.extend_from_slice(&height.to_le_bytes());
        f.push(1); // depth
        f.push(20); // format: Bgra8
        f.push(0); // resource type
        f.push(0); // flags
        f.extend_from_slice(pixels_bgra);
        f
    }

    #[test]
    fn exposes_property_interfaces() {
        let unknown: IUnknown = CTexPropertyHandler::new().into();
        assert!(unknown.cast::<IInitializeWithStream>().is_ok());
        assert!(unknown.cast::<IPropertyStore>().is_ok());
    }

    #[test]
    fn populates_properties_from_stream() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        let file = bgra8_tex(2, 2, &[0xFF; 16]);
        let stream = unsafe { SHCreateMemStream(Some(&file)) }.expect("mem stream");

        let unknown: IUnknown = CTexPropertyHandler::new().into();
        let init: IInitializeWithStream = unknown.cast().unwrap();
        unsafe { init.Initialize(&stream, 0).unwrap() };

        let store: IPropertyStore = unknown.cast().unwrap();
        let count = unsafe { store.GetCount().unwrap() };
        assert!(count >= 6, "expected >= 6 properties, got {count}");
    }
}

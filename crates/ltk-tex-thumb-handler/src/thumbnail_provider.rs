// =============================================================================
// TEX THUMBNAIL PROVIDER (following Microsoft's CRecipeThumbProvider pattern)
// =============================================================================

use std::ffi::c_void;
use std::sync::atomic::AtomicI32;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Com::IStream;
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTS_ALPHATYPE, WTSAT_RGB,
};
use windows::core::*;

use crate::image_processing::*;

#[implement(IInitializeWithStream, IThumbnailProvider)]
pub struct CTexThumbProvider {
    #[allow(dead_code)]
    cRef: AtomicI32,
    pStream: std::sync::Mutex<Option<IStream>>,
}

impl Default for CTexThumbProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CTexThumbProvider {
    pub fn new() -> Self {
        Self {
            cRef: AtomicI32::new(1),
            pStream: std::sync::Mutex::new(None),
        }
    }
}

impl IInitializeWithStream_Impl for CTexThumbProvider_Impl {
    fn Initialize(&self, pStream: Option<&IStream>, _grfMode: u32) -> Result<()> {
        // Can only be initialized once
        let mut stream_guard = self.pStream.lock().unwrap();
        if stream_guard.is_some() {
            return Err(Error::from(E_UNEXPECTED));
        }

        // Take a reference to the stream
        if let Some(stream) = pStream {
            *stream_guard = Some(stream.clone());
            Ok(())
        } else {
            Err(Error::from(E_INVALIDARG))
        }
    }
}

impl IThumbnailProvider_Impl for CTexThumbProvider_Impl {
    fn GetThumbnail(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwAlpha: *mut WTS_ALPHATYPE,
    ) -> Result<()> {
        let stream_guard = self.pStream.lock().unwrap();
        let stream = stream_guard.as_ref().ok_or(Error::from(E_UNEXPECTED))?;

        let bytes = unsafe { read_stream_to_bytes(stream)? };
        let (rgba, width, height) = decode_tex_file(&bytes)?;
        let (scaled_rgba, scaled_w, scaled_h) = scale_image(&rgba, width, height, cx);
        let hbmp = unsafe { create_hbitmap_from_rgba(&scaled_rgba, scaled_w, scaled_h)? };

        unsafe {
            *phbmp = hbmp;
            *pdwAlpha = WTSAT_RGB; // Not using premultiplied alpha
        }

        Ok(())
    }
}

pub fn CTexThumbProvider_CreateInstance(riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    let provider = CTexThumbProvider::new();
    let unknown: IUnknown = provider.into();
    unsafe { unknown.query(riid, ppv) }
}

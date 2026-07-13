// =============================================================================
// TEX PREVIEW HANDLER (Explorer preview pane / Alt+P)
//
// Renders the decoded .tex full-resolution over an alpha checkerboard, with a
// metadata overlay (dimensions, format, mip count, alpha). Runs in prevhost.exe.
// =============================================================================

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::Once;
use std::sync::atomic::AtomicI32;

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::IStream;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{IObjectWithSite, IObjectWithSite_Impl};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IPreviewHandler, IPreviewHandler_Impl, IPreviewHandlerVisuals, IPreviewHandlerVisuals_Impl,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::debug::debug_log;
use crate::image_processing::{TexMeta, decode_tex_with_meta};
use crate::raster::{draw_checker, draw_overlay, resize_rgba};
use crate::utils::{create_premul_hbitmap, read_stream_to_bytes, to_premultiplied_bgra};

const WNDCLASS_NAME: PCWSTR = w!("LtkTexPreviewWnd");
static REGISTER_CLASS: Once = Once::new();

// A texture is not a document: we ignore the host's pane colors (Explorer hands
// us white even in dark mode) and always render on a fixed dark canvas.
// COLORREF is 0x00BBGGRR.
const PANE_BG: COLORREF = COLORREF(0x001E_1E1E);
const PANE_TEXT: COLORREF = COLORREF(0x00F0_F0F0);

/// Everything the WNDPROC needs to paint, owned by the COM object and handed to
/// the window as a raw pointer (never locks the object's mutex → no re-entrancy).
struct PaintData {
    /// Full-resolution, non-premultiplied RGBA. Resized per-paint to the display
    /// size so scaling uses a smooth filter instead of AlphaBlend's nearest tap.
    rgba: Vec<u8>,
    width: i32,
    height: i32,
    lines: Vec<String>,
}

struct PreviewState {
    stream: Option<IStream>,
    site: Option<IUnknown>,
    parent: HWND,
    rect: RECT,
    hwnd: HWND,
    paint: Option<Box<PaintData>>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            stream: None,
            site: None,
            parent: HWND(std::ptr::null_mut()),
            rect: RECT::default(),
            hwnd: HWND(std::ptr::null_mut()),
            paint: None,
        }
    }
}

#[implement(
    IInitializeWithStream,
    IObjectWithSite,
    IPreviewHandler,
    IPreviewHandlerVisuals
)]
pub struct CTexPreviewHandler {
    #[allow(dead_code)]
    cRef: AtomicI32,
    state: Mutex<PreviewState>,
}

impl Default for CTexPreviewHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CTexPreviewHandler {
    pub fn new() -> Self {
        Self {
            cRef: AtomicI32::new(1),
            state: Mutex::new(PreviewState::default()),
        }
    }
}

impl Drop for CTexPreviewHandler {
    fn drop(&mut self) {
        // Ensure the hosting window is torn down even if Unload was skipped.
        if let Ok(mut st) = self.state.lock() {
            let hwnd = st.hwnd;
            st.hwnd = HWND(std::ptr::null_mut());
            let paint = st.paint.take();
            drop(st);
            if !hwnd.0.is_null() {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            drop(paint);
        }
    }
}

impl IInitializeWithStream_Impl for CTexPreviewHandler_Impl {
    fn Initialize(&self, pstream: Option<&IStream>, _grfmode: u32) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        if st.stream.is_some() {
            return Err(Error::from(E_UNEXPECTED));
        }
        let stream = pstream.ok_or(Error::from(E_INVALIDARG))?;
        st.stream = Some(stream.clone());
        Ok(())
    }
}

impl IObjectWithSite_Impl for CTexPreviewHandler_Impl {
    fn SetSite(&self, punksite: Option<&IUnknown>) -> Result<()> {
        self.state.lock().unwrap().site = punksite.cloned();
        Ok(())
    }

    fn GetSite(&self, riid: *const GUID, ppvsite: *mut *mut c_void) -> Result<()> {
        let st = self.state.lock().unwrap();
        match &st.site {
            Some(site) => unsafe { site.query(riid, ppvsite).ok() },
            None => Err(Error::from(E_FAIL)),
        }
    }
}

impl IPreviewHandler_Impl for CTexPreviewHandler_Impl {
    fn SetWindow(&self, hwnd: HWND, prc: *const RECT) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        st.parent = hwnd;
        if !prc.is_null() {
            st.rect = unsafe { *prc };
        }
        let (child, parent, rc) = (st.hwnd, st.parent, st.rect);
        drop(st);
        if !child.0.is_null() {
            unsafe {
                let _ = SetParent(child, parent);
                let _ = MoveWindow(
                    child,
                    rc.left,
                    rc.top,
                    rc.right - rc.left,
                    rc.bottom - rc.top,
                    true,
                );
            }
        }
        Ok(())
    }

    fn SetRect(&self, prc: *const RECT) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        if !prc.is_null() {
            st.rect = unsafe { *prc };
        }
        let (child, rc) = (st.hwnd, st.rect);
        drop(st);
        if !child.0.is_null() {
            unsafe {
                let _ = MoveWindow(
                    child,
                    rc.left,
                    rc.top,
                    rc.right - rc.left,
                    rc.bottom - rc.top,
                    true,
                );
            }
        }
        Ok(())
    }

    fn DoPreview(&self) -> Result<()> {
        let mut st = self.state.lock().unwrap();

        let stream = st.stream.clone().ok_or(Error::from(E_UNEXPECTED))?;
        let bytes = unsafe { read_stream_to_bytes(&stream) }?;
        let (rgba, w, h, meta) = decode_tex_with_meta(&bytes).inspect_err(|e| {
            debug_log(&format!("preview: decode failed: {e:?}"));
        })?;

        let paint = Box::new(PaintData {
            rgba,
            width: w as i32,
            height: h as i32,
            lines: build_meta_lines(&meta),
        });

        ensure_window_class();

        let (parent, rc) = (st.parent, st.rect);
        let paint_ptr: *const PaintData = &*paint;

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WNDCLASS_NAME,
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                rc.left,
                rc.top,
                rc.right - rc.left,
                rc.bottom - rc.top,
                parent,
                HMENU(std::ptr::null_mut()),
                instance(),
                Some(paint_ptr as *const c_void),
            )
        }?;

        st.hwnd = hwnd;
        st.paint = Some(paint);
        debug_log(&format!("preview: DoPreview ok {w}x{h}"));
        Ok(())
    }

    fn Unload(&self) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        let hwnd = st.hwnd;
        st.hwnd = HWND(std::ptr::null_mut());
        st.stream = None;
        let paint = st.paint.take();
        drop(st);
        // Destroy the window BEFORE freeing PaintData so no WM_PAINT can read it.
        if !hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        drop(paint);
        Ok(())
    }

    fn SetFocus(&self) -> Result<()> {
        let hwnd = self.state.lock().unwrap().hwnd;
        if !hwnd.0.is_null() {
            unsafe {
                let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(hwnd);
            }
        }
        Ok(())
    }

    fn QueryFocus(&self) -> Result<HWND> {
        Ok(unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetFocus() })
    }

    fn TranslateAccelerator(&self, _pmsg: *const MSG) -> Result<()> {
        // We have no accelerators; S_FALSE tells the host to keep routing.
        Err(Error::from(S_FALSE))
    }
}

impl IPreviewHandlerVisuals_Impl for CTexPreviewHandler_Impl {
    // Intentionally ignored: we render on our own fixed dark canvas regardless of
    // the pane theme the host requests (see PANE_BG / PANE_TEXT).
    fn SetBackgroundColor(&self, _color: COLORREF) -> Result<()> {
        Ok(())
    }

    fn SetFont(&self, _plogfont: *const LOGFONTW) -> Result<()> {
        Ok(())
    }

    fn SetTextColor(&self, _color: COLORREF) -> Result<()> {
        Ok(())
    }
}

fn build_meta_lines(m: &TexMeta) -> Vec<String> {
    vec![
        format!("{} x {}", m.width, m.height),
        format!("Format: {}", m.format),
        format!("Mips: {}", m.mip_count),
        format!("Alpha: {}", if m.has_alpha { "yes" } else { "no" }),
    ]
}

fn instance() -> HINSTANCE {
    unsafe { HINSTANCE(GetModuleHandleW(PCWSTR::null()).unwrap_or_default().0) }
}

fn ensure_window_class() {
    REGISTER_CLASS.call_once(|| unsafe {
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: WNDCLASS_NAME,
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1), // painted in WM_PAINT (double-buffered)
            WM_PAINT => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const PaintData;
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if !ptr.is_null() {
                    paint(hwnd, hdc, &*ptr);
                }
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Double-buffered paint: background fill → checkerboard → alpha-blended texture
/// (fit to pane, aspect preserved) → metadata overlay.
fn paint(hwnd: HWND, hdc: HDC, pd: &PaintData) {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let cw = (rc.right - rc.left).max(1);
        let ch = (rc.bottom - rc.top).max(1);

        let mem = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, cw, ch);
        let old_bmp = SelectObject(mem, bmp);

        // Background (fixed dark canvas, not the host's pane color)
        let bg_brush = CreateSolidBrush(PANE_BG);
        FillRect(
            mem,
            &RECT {
                left: 0,
                top: 0,
                right: cw,
                bottom: ch,
            },
            bg_brush,
        );
        let _ = DeleteObject(bg_brush);

        // Fit rect (aspect-preserving, allows upscaling small textures)
        let (iw, ih) = (pd.width.max(1), pd.height.max(1));
        let scale = f64::min(cw as f64 / iw as f64, ch as f64 / ih as f64);
        let dw = ((iw as f64 * scale).round() as i32).clamp(1, cw);
        let dh = ((ih as f64 * scale).round() as i32).clamp(1, ch);
        let dx = (cw - dw) / 2;
        let dy = (ch - dh) / 2;

        draw_checker(mem, dx, dy, dw, dh);

        // Resize to the display size with a smooth filter (AlphaBlend only does a
        // nearest tap), then blend the result 1:1 over the checkerboard.
        let display_rgba = resize_rgba(&pd.rgba, iw, ih, dw, dh);
        let premul = to_premultiplied_bgra(&display_rgba);
        if let Ok(hbmp) = create_premul_hbitmap(&premul, dw as u32, dh as u32) {
            let src = CreateCompatibleDC(hdc);
            let old_src = SelectObject(src, hbmp);
            let bf = BLENDFUNCTION {
                BlendOp: 0, // AC_SRC_OVER
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 1, // AC_SRC_ALPHA (premultiplied)
            };
            let _ = AlphaBlend(mem, dx, dy, dw, dh, src, 0, 0, dw, dh, bf);
            SelectObject(src, old_src);
            let _ = DeleteDC(src);
            let _ = DeleteObject(hbmp);
        }

        draw_overlay(mem, ch, &pd.lines, PANE_TEXT);

        let _ = BitBlt(hdc, 0, 0, cw, ch, mem, 0, 0, SRCCOPY);

        SelectObject(mem, old_bmp);
        let _ = DeleteObject(bmp);
        let _ = DeleteDC(mem);
    }
}

// Rasterization leaf helpers (resample, checkerboard, overlay) live in `raster`.

pub fn CTexPreviewHandler_CreateInstance(riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    let handler = CTexPreviewHandler::new();
    let unknown: IUnknown = handler.into();
    unsafe { unknown.query(riid, ppv) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_all_preview_interfaces() {
        let unknown: IUnknown = CTexPreviewHandler::new().into();
        assert!(unknown.cast::<IInitializeWithStream>().is_ok());
        assert!(unknown.cast::<IObjectWithSite>().is_ok());
        assert!(unknown.cast::<IPreviewHandler>().is_ok());
        assert!(unknown.cast::<IPreviewHandlerVisuals>().is_ok());
    }

    #[test]
    fn builds_expected_metadata_lines() {
        let meta = TexMeta {
            format: "BC7",
            width: 1024,
            height: 512,
            mip_count: 11,
            has_alpha: true,
        };
        assert_eq!(
            build_meta_lines(&meta),
            vec![
                "1024 x 512".to_string(),
                "Format: BC7".to_string(),
                "Mips: 11".to_string(),
                "Alpha: yes".to_string(),
            ]
        );
    }
}

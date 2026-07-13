// =============================================================================
// UTILITIES
//
// Generic, non-TEX helpers shared across handlers: IStream I/O, pixel-buffer
// math (premultiply, scale), and 32bpp DIB creation. Nothing here knows about
// the .tex format.
// =============================================================================

use std::ffi::c_void;
use std::{mem, ptr};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::IStream;
use windows::core::*;

/// Read all bytes from IStream
pub unsafe fn read_stream_to_bytes(stream: &IStream) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(64 * 1024);
    loop {
        let mut chunk = [0u8; 64 * 1024];
        let mut read = 0u32;
        let hr = unsafe {
            stream.Read(
                chunk.as_mut_ptr() as *mut c_void,
                chunk.len() as u32,
                Some(&mut read),
            )
        };
        if !hr.is_ok() {
            return Err(Error::from(hr));
        }
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read as usize]);
    }
    Ok(buf)
}

/// Build a top-down, premultiplied BGRA buffer suitable for `AlphaBlend`.
///
/// `AlphaBlend` with `AC_SRC_ALPHA` requires premultiplied color channels; this
/// swaps R/B and multiplies RGB by A so the texture composites correctly over
/// the checkerboard backing.
pub fn to_premultiplied_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for (dst, src) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        let (r, g, b, a) = (src[0] as u32, src[1] as u32, src[2] as u32, src[3] as u32);
        dst[0] = ((b * a + 127) / 255) as u8;
        dst[1] = ((g * a + 127) / 255) as u8;
        dst[2] = ((r * a + 127) / 255) as u8;
        dst[3] = a as u8;
    }
    out
}

/// Scale RGBA image to fit within thumbnail size
pub fn scale_image(src: &[u8], src_w: u32, src_h: u32, cx: u32) -> (Vec<u8>, u32, u32) {
    let (dst_w, dst_h) = if src_w >= src_h {
        (cx, (src_h * cx + src_w / 2) / src_w)
    } else {
        ((src_w * cx + src_h / 2) / src_h, cx)
    };

    let mut out = vec![0u8; (dst_w * dst_h * 4) as usize];
    for y in 0..dst_h {
        let sy = (y * src_h / dst_h).clamp(0, src_h - 1);
        for x in 0..dst_w {
            let sx = (x * src_w / dst_w).clamp(0, src_w - 1);
            let si = ((sy * src_w + sx) * 4) as usize;
            let di = ((y * dst_w + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    (out, dst_w, dst_h)
}

/// Create a top-down 32bpp DIB section HBITMAP from already-premultiplied BGRA
/// bytes, for use as an `AlphaBlend` source in the preview handler.
pub unsafe fn create_premul_hbitmap(
    bgra_premul: &[u8],
    width: u32,
    height: u32,
) -> Result<HBITMAP> {
    let mut bi: BITMAPV5HEADER = unsafe { mem::zeroed() };
    bi.bV5Size = mem::size_of::<BITMAPV5HEADER>() as u32;
    bi.bV5Width = width as i32;
    bi.bV5Height = -(height as i32); // Top-down DIB
    bi.bV5Planes = 1;
    bi.bV5BitCount = 32;
    bi.bV5Compression = BI_RGB;

    let mut bits: *mut c_void = ptr::null_mut();
    let hbmp = unsafe {
        CreateDIBSection(
            HDC(std::ptr::null_mut()),
            &bi as *const _ as *const BITMAPINFO,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?
    };

    if hbmp.is_invalid() || bits.is_null() {
        return Err(Error::from(E_FAIL));
    }

    let n = (width * height * 4) as usize;
    let dst = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, n) };
    dst.copy_from_slice(&bgra_premul[..n]);

    Ok(hbmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiply_swaps_to_bgra_and_scales_by_alpha() {
        // RGBA red at 50% alpha -> premultiplied BGRA.
        let rgba = [255u8, 0, 0, 128];
        let out = to_premultiplied_bgra(&rgba);
        // b=0, g=0, r=(255*128+127)/255=128, a=128
        assert_eq!(out, vec![0, 0, 128, 128]);
    }
}

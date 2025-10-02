// =============================================================================
// IMAGE PROCESSING AND BITMAP OPERATIONS
// =============================================================================

use league_toolkit::texture::Tex;
use std::ffi::c_void;
use std::io::Cursor;
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

/// Decode TEX file to RGBA image data
pub fn decode_tex_file(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let mut cursor = Cursor::new(bytes);
    let tex = Tex::from_reader(&mut cursor).map_err(|_| Error::from(E_FAIL))?;
    let image = tex.decode_mipmap(0).map_err(|_| Error::from(E_FAIL))?;
    let rgba = image.into_rgba_image().map_err(|_| Error::from(E_FAIL))?;
    let width = rgba.width();
    let height = rgba.height();
    let data = rgba.into_raw();
    Ok((data, width, height))
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

/// Convert RGBA to premultiplied BGRA HBITMAP (following Microsoft's ConvertBitmapSourceTo32BPPHBITMAP pattern)
pub unsafe fn create_hbitmap_from_rgba(rgba: &[u8], width: u32, height: u32) -> Result<HBITMAP> {
    let mut bi: BITMAPV5HEADER = unsafe { mem::zeroed() };
    bi.bV5Size = mem::size_of::<BITMAPV5HEADER>() as u32;
    bi.bV5Width = width as i32;
    bi.bV5Height = -(height as i32); // Top-down DIB
    bi.bV5Planes = 1;
    bi.bV5BitCount = 32;
    bi.bV5Compression = BI_RGB; // Use standard RGB format (which is actually BGRA in memory)

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

    let dst =
        unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, (width * height * 4) as usize) };

    for i in 0..(width * height) as usize {
        let r = rgba[i * 4];
        let g = rgba[i * 4 + 1];
        let b = rgba[i * 4 + 2];
        let a = rgba[i * 4 + 3];

        // Write as BGRA for Windows (just swap R and B, no premultiplication)
        dst[i * 4] = b;
        dst[i * 4 + 1] = g;
        dst[i * 4 + 2] = r;
        dst[i * 4 + 3] = a;
    }

    Ok(hbmp)
}

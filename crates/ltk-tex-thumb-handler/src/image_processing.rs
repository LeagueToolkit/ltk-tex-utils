// =============================================================================
// IMAGE PROCESSING AND BITMAP OPERATIONS
// =============================================================================

use ltk_texture::Tex;
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

/// Human-facing metadata about a decoded TEX, for the preview overlay.
pub struct TexMeta {
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
    pub has_alpha: bool,
}

fn format_name(format: ltk_texture::tex::Format) -> &'static str {
    use ltk_texture::tex::Format;
    match format {
        Format::Etc1 => "ETC1",
        Format::Etc2Eac => "ETC2/EAC",
        Format::Bc1 => "BC1",
        Format::Bc3 => "BC3",
        Format::Bc7 => "BC7",
        Format::Bc5Snorm => "BC5 (snorm)",
        Format::Bgra8 => "BGRA8",
        Format::Rgba16Float => "RGBA16F",
        Format::Rgba32Float => "RGBA32F",
    }
}

/// Decode a TEX file to full-resolution RGBA plus header metadata for the preview.
pub fn decode_tex_with_meta(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32, TexMeta)> {
    let mut cursor = Cursor::new(bytes);
    let tex = Tex::from_reader(&mut cursor).map_err(|_| Error::from(E_FAIL))?;

    let format = format_name(tex.format);
    let mip_count = tex.mip_count;

    let image = tex.decode_mipmap(0).map_err(|_| Error::from(E_FAIL))?;
    let rgba = image.into_rgba_image().map_err(|_| Error::from(E_FAIL))?;
    let width = rgba.width();
    let height = rgba.height();
    let data = rgba.into_raw();

    // Report alpha based on actual decoded content, not just the format.
    let has_alpha = data.chunks_exact(4).any(|px| px[3] != 0xFF);

    let meta = TexMeta {
        format,
        width,
        height,
        mip_count,
        has_alpha,
    };
    Ok((data, width, height, meta))
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

/// Create a top-down 32bpp DIB section HBITMAP from already-premultiplied BGRA
/// bytes, for use as an `AlphaBlend` source in the preview handler.
pub unsafe fn create_premul_hbitmap(bgra_premul: &[u8], width: u32, height: u32) -> Result<HBITMAP> {
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

    /// Minimal uncompressed BGRA8 .tex (format id 20), no mipmaps.
    fn bgra8_tex(width: u16, height: u16, pixels_bgra: &[u8]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(b"TEX\0");
        f.extend_from_slice(&width.to_le_bytes());
        f.extend_from_slice(&height.to_le_bytes());
        f.push(1); // depth
        f.push(20); // format: Bgra8
        f.push(0); // resource type: texture
        f.push(0); // flags: no mipmaps
        f.extend_from_slice(pixels_bgra);
        f
    }

    #[test]
    fn decode_with_meta_reports_dimensions_format_and_alpha() {
        // 2x2 BGRA8: pixel 0 is red at 50% alpha, the rest opaque white.
        let px = [
            0x00, 0x00, 0xFF, 0x80, // BGRA red, a=128
            0xFF, 0xFF, 0xFF, 0xFF, //
            0xFF, 0xFF, 0xFF, 0xFF, //
            0xFF, 0xFF, 0xFF, 0xFF, //
        ];
        let file = bgra8_tex(2, 2, &px);

        let (rgba, w, h, meta) = decode_tex_with_meta(&file).expect("decode");
        assert_eq!((w, h), (2, 2));
        assert_eq!(meta.format, "BGRA8");
        assert_eq!(meta.mip_count, 1);
        assert!(meta.has_alpha, "a=128 pixel should be detected");
        // First decoded pixel is red with alpha 128.
        assert_eq!(&rgba[0..4], &[0xFF, 0x00, 0x00, 0x80]);
    }

    #[test]
    fn premultiply_swaps_to_bgra_and_scales_by_alpha() {
        // RGBA red at 50% alpha -> premultiplied BGRA.
        let rgba = [255u8, 0, 0, 128];
        let out = to_premultiplied_bgra(&rgba);
        // b=0, g=0, r=(255*128+127)/255=128, a=128
        assert_eq!(out, vec![0, 0, 128, 128]);
    }
}

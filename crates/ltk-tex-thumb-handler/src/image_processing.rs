// =============================================================================
// TEX DECODING
//
// Decoding of the League .tex format into RGBA plus header metadata. Generic
// pixel/DIB/stream helpers live in `utils`.
// =============================================================================

use ltk_texture::Tex;
use std::io::Cursor;
use windows::Win32::Foundation::*;
use windows::core::*;

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
}

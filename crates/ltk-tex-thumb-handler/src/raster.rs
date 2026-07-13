// =============================================================================
// RASTERIZATION HELPERS
//
// Stateless leaf routines that support the preview handler's paint pass:
// resampling to the display size, the alpha checkerboard, and the metadata text
// overlay. Each draws onto a caller-provided HDC (or transforms a raw buffer)
// and knows nothing about the preview handler's COM/window state.
// =============================================================================

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;

/// Resample full-res RGBA to the display size. CatmullRom keeps upscaled
/// textures crisp; Triangle avoids ringing when shrinking.
pub fn resize_rgba(rgba: &[u8], src_w: i32, src_h: i32, dst_w: i32, dst_h: i32) -> Vec<u8> {
    if (dst_w, dst_h) == (src_w, src_h) {
        return rgba.to_vec();
    }
    match image::RgbaImage::from_raw(src_w as u32, src_h as u32, rgba.to_vec()) {
        Some(img) => {
            let filter = if dst_w >= src_w {
                image::imageops::FilterType::CatmullRom
            } else {
                image::imageops::FilterType::Triangle
            };
            image::imageops::resize(&img, dst_w as u32, dst_h as u32, filter).into_raw()
        }
        None => rgba.to_vec(),
    }
}

/// Alpha checkerboard so transparent regions read clearly.
pub fn draw_checker(hdc: HDC, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        const CELL: i32 = 8;
        let light = CreateSolidBrush(COLORREF(0x009E_9E9E));
        let dark = CreateSolidBrush(COLORREF(0x006E_6E6E));
        let mut yy = 0;
        while yy < h {
            let mut xx = 0;
            while xx < w {
                let cw = CELL.min(w - xx);
                let cyh = CELL.min(h - yy);
                let brush = if ((xx / CELL) + (yy / CELL)) % 2 == 0 {
                    light
                } else {
                    dark
                };
                FillRect(
                    hdc,
                    &RECT {
                        left: x + xx,
                        top: y + yy,
                        right: x + xx + cw,
                        bottom: y + yy + cyh,
                    },
                    brush,
                );
                xx += CELL;
            }
            yy += CELL;
        }
        let _ = DeleteObject(light);
        let _ = DeleteObject(dark);
    }
}

/// Metadata lines in the bottom-left, with a 1px shadow for legibility.
pub fn draw_overlay(hdc: HDC, ch: i32, lines: &[String], text_color: COLORREF) {
    if lines.is_empty() {
        return;
    }
    unsafe {
        let font = GetStockObject(DEFAULT_GUI_FONT);
        let old_font = SelectObject(hdc, font);
        SetBkMode(hdc, TRANSPARENT);

        const PAD: i32 = 8;
        const LINE_H: i32 = 18;
        let mut y = ch - PAD - LINE_H * lines.len() as i32;
        for line in lines {
            let wide: Vec<u16> = line.encode_utf16().collect();
            SetTextColor(hdc, COLORREF(0x0000_0000));
            let _ = TextOutW(hdc, PAD + 1, y + 1, &wide);
            SetTextColor(hdc, text_color);
            let _ = TextOutW(hdc, PAD, y, &wide);
            y += LINE_H;
        }

        SelectObject(hdc, old_font);
    }
}

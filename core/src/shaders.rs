use crate::lcd::{LCD, LCDH, LCDW, LCD_BUFFER_SIZE};

pub fn normal(frame: &[u32; LCD_BUFFER_SIZE], out: &mut [u8], scale: usize) {
    for (frame_row, out_block) in frame.chunks_exact(LCDW).zip(out.chunks_exact_mut(LCDW * scale * scale * 4)) {
        // Horizontal scaling: copy src row on first out row
        for (frame_px, out_px_block) in frame_row.iter().zip(out_block[..LCDW * scale * 4].chunks_exact_mut(4 * scale)) {
            for out_px in out_px_block.chunks_exact_mut(4) {
                out_px.copy_from_slice(&frame_px.to_be_bytes());
            }
        }
        // Verical scaling: copy first dst_row on other dst_rows
        for out_y in 1..scale {
            out_block.copy_within(..LCDW * scale * 4, out_y * LCDW * scale * 4);
        }
    }
}

pub fn drop_shadow(
    background: &[u32; LCD_BUFFER_SIZE],
    foreground: &[u32; LCD_BUFFER_SIZE],
    out: &mut [u8],
    scale: usize,
    offset_x: i16,
    offset_y: i16,
) {
    for x in 0..(LCDW as i16) {
        for y in 0..(LCDH as i16) {
            let idx = LCD::to_idx(x as usize, y as usize, 1, 0, 0);
            let mut rgba: [u8; 4];
            if foreground[idx] != 0 {
                rgba = foreground[idx].to_be_bytes();
            } else {
                rgba = background[idx].to_be_bytes();
                let (shadow_ref_x, shadow_ref_y) = (x - offset_x, y - offset_y);
                if 0 <= shadow_ref_x && shadow_ref_x < LCDW as i16 && 0 <= shadow_ref_y && shadow_ref_y < LCDH as i16 {
                    let shadow_ref_idx = LCD::to_idx(shadow_ref_x as usize, shadow_ref_y as usize, 1, 0, 0);
                    if foreground[shadow_ref_idx] != 0 {
                        rgba.iter_mut().take(3).for_each(|c| *c = *c / 4 * 3);
                    }
                }
            }
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = 4 * LCD::to_idx(x as usize, y as usize, scale, dx, dy);
                    out[idx..idx + 4].copy_from_slice(&rgba);
                }
            }
        }
    }
}

pub fn anaglyph_3d(
    background: &[u32; LCD_BUFFER_SIZE],
    foreground: &[u32; LCD_BUFFER_SIZE],
    out: &mut [u8],
    scale: usize,
    offset_background: usize,
    offset_foreground: usize,
) {
    for xr in 0..LCDW {
        for y in 0..LCDH {
            // Retrieve right pixel from original frame
            let idxr: usize = LCD::to_idx(xr, y, 1, 0, 0);
            let pxr = if foreground[idxr] != 0 {
                foreground[idxr]
            } else {
                background[idxr]
            };
            // Retrieve left pixel with a different displacement for foreground and background
            let (xl_bg, xl_fg) = (xr + offset_background, xr + offset_foreground);
            let idxl_fg = LCD::to_idx(xl_fg, y, 1, 0, 0);
            let pxl = if xl_fg < LCDW && foreground[idxl_fg] != 0 {
                foreground[idxl_fg]
            } else if xl_bg < LCDW {
                let idxl_bg = LCD::to_idx(xl_bg, y, 1, 0, 0);
                background[idxl_bg]
            } else {
                0
            };
            // Mix channels, GB from right pixel and R from left pixel
            let rgba: [u8; 4] = ((pxr & 0x00FFFFFF) | (pxl & 0xFF0000FF)).to_be_bytes();
            // Source: https://www.3dtv.at/knowhow/anaglyphcomparison_en.aspx
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = 4 * LCD::to_idx(xr, y, scale, dx, dy);
                    out[idx..idx + 4].copy_from_slice(&rgba);
                }
            }
        }
    }
}

pub fn lcd(frame: &[u32; LCD_BUFFER_SIZE], out: &mut [u8], scale: usize, dmg_bg_palette: Option<u32>) {
    for x in 0..LCDW {
        for y in 0..LCDH {
            let idx = LCD::to_idx(x, y, 1, 0, 0);
            let px = frame[idx];
            let rgba = px.to_be_bytes();
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = 4 * LCD::to_idx(x, y, scale, dx, dy);
                    let mut rgba = rgba;
                    if let Some(dmg_bg_palette) = dmg_bg_palette {
                        // DMG mode
                        if px == dmg_bg_palette && x < LCDW - 1 && y > 0 && frame[LCD::to_idx(x + 1, y - 1, 1, 0, 0)] != dmg_bg_palette {
                            // Draw drop shadow if pixel is background and is covered by foreground
                            rgba.iter_mut().take(3).for_each(|c| *c = c.saturating_sub(0x08));
                        } else if px != dmg_bg_palette && scale >= 2 && (dx == 0 || dy == 0) {
                            // Draw light grid on non-background pixels
                            rgba.iter_mut().take(3).for_each(|c| *c = c.saturating_add(0x0F));
                        }
                    } else {
                        // GBC mode
                        if scale >= 2 && (dx == 0 || dy == 0) {
                            // Draw grid for colored LCD
                            rgba.iter_mut().take(3).for_each(|c| *c = c.saturating_sub(*c / 3));
                        }
                    }
                    out[idx..idx + 4].copy_from_slice(&rgba);
                }
            }
        }
    }
}

pub fn crt(frame: &[u32; LCD_BUFFER_SIZE], out: &mut [u8], scale: usize) {
    for x in 0..LCDW {
        for y in 0..LCDH {
            let idx = LCD::to_idx(x, y, 1, 0, 0);
            let rgba = frame[idx].to_be_bytes();
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = 4 * LCD::to_idx(x, y, scale, dx, dy);
                    let mut rgba = rgba;
                    if scale >= 2 && dy == scale - 1 {
                        // Draw scanlines
                        rgba.iter_mut().take(3).for_each(|c| *c /= 3);
                    } else if scale >= 3 && dx == 0 {
                        // Draw left chromatic aberration
                        rgba[0] = rgba[0].saturating_add(0x30);
                    } else if scale >= 3 && dx == scale - 1 {
                        // Draw right chromatic aberration
                        rgba[2] = rgba[2].saturating_add(0x30);
                    }
                    out[idx..idx + 4].copy_from_slice(&rgba);
                }
            }
        }
    }
}

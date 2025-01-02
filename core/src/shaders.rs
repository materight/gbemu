use crate::lcd::{LCD, LCDH, LCDW};

pub fn normal(buffer: &LCD, out: &mut [u8], scale: usize) {
    for x in 0..(LCDW as u8) {
        for y in 0..(LCDH as u8) {
            let idx = LCD::to_idx(x, y, 1, 0, 0);
            let [_, r, g, b] = buffer.frame[idx].to_be_bytes();
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = LCD::to_idx(x, y, scale, dx, dy);
                    out[idx * 4..idx * 4 + 4].copy_from_slice(&[r, g, b, 0xFF]);
                }
            }
        }
    }
}

pub fn drop_shadow(buffer: &LCD, out: &mut [u8], scale: usize, offset_x: i16, offset_y: i16) {
    for x in 0..(LCDW as i16) {
        for y in 0..(LCDH as i16) {
            let idx = LCD::to_idx(x as u8, y as u8, 1, 0, 0);
            let [mut r, mut g, mut b]: [u8; 3];
            if buffer.foreground[idx] != 0 {
                [_, r, g, b] = buffer.foreground[idx].to_be_bytes();
            } else {
                [_, r, g, b] = buffer.background[idx].to_be_bytes();
                let (shadow_ref_x, shadow_ref_y) = (x - offset_x, y - offset_y);
                if 0 <= shadow_ref_x && shadow_ref_x < LCDW as i16 && 0 <= shadow_ref_y && shadow_ref_y < LCDH as i16 {
                    let shadow_ref_idx = LCD::to_idx(shadow_ref_x as u8, shadow_ref_y as u8, 1, 0, 0);
                    if buffer.foreground[shadow_ref_idx] != 0 {
                        [r, g, b] = [r / 4 * 3, g / 4 * 3, b / 4 * 3];
                    }
                }
            }
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = LCD::to_idx(x as u8, y as u8, scale, dx, dy);
                    out[idx * 4..idx * 4 + 4].copy_from_slice(&[r, g, b, 0xFF]);
                }
            }
        }
    }
}

pub fn anaglyph_3d(buffer: &LCD, out: &mut [u8], scale: usize, offset_background: u8, offset_foreground: u8) {
    for xr in 0..(LCDW as u8) {
        for y in 0..(LCDH as u8) {
            // Retrieve right pixel from original frame
            let idxr: usize = LCD::to_idx(xr, y, 1, 0, 0);
            let pxr = if buffer.foreground[idxr] != 0 {
                buffer.foreground[idxr]
            } else {
                buffer.background[idxr]
            };
            let [_, _, gr, br] = pxr.to_be_bytes();
            // Retrieve left pixel with a different displacement for foreground and background
            let (xl_bg, xl_fg) = (xr + offset_background, xr + offset_foreground);
            let idxl_fg = LCD::to_idx(xl_fg, y, 1, 0, 0);
            let pxl = if xl_fg < LCDW as u8 && buffer.foreground[idxl_fg] != 0 {
                buffer.foreground[idxl_fg]
            } else if xl_bg < LCDW as u8 {
                let idxl_bg = LCD::to_idx(xl_bg, y, 1, 0, 0);
                buffer.background[idxl_bg]
            } else {
                0
            };
            let [_, rl, _, _] = pxl.to_be_bytes();
            // Source: https://www.3dtv.at/knowhow/anaglyphcomparison_en.aspx
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = LCD::to_idx(xr, y, scale, dx, dy);
                    out[idx * 4..idx * 4 + 4].copy_from_slice(&[rl, gr, br, 0xFF]);
                }
            }
        }
    }
}

pub fn lcd(buffer: &LCD, out: &mut [u8], scale: usize, dmg_bg_palette: Option<u32>) {
    for x in 0..(LCDW as u8) {
        for y in 0..(LCDH as u8) {
            let idx = LCD::to_idx(x, y, 1, 0, 0);
            let px = buffer.frame[idx];
            let [_, r, g, b] = px.to_be_bytes();
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = LCD::to_idx(x, y, scale, dx, dy);
                    if let Some(dmg_bg_palette) = dmg_bg_palette {
                        // DMG mode
                        if px == dmg_bg_palette
                            && x < LCDW as u8 - 1
                            && y > 0
                            && buffer.frame[LCD::to_idx(x + 1, y - 1, 1, 0, 0)] != dmg_bg_palette
                        {
                            // Draw drop shadow if pixel is background and is covered by foreground
                            out[idx * 4..idx * 4 + 4].copy_from_slice(&[
                                r.saturating_sub(0x08),
                                g.saturating_sub(0x08),
                                b.saturating_sub(0x08),
                                0xFF,
                            ]);
                        } else if px != dmg_bg_palette && scale >= 2 && (dx == 0 || dy == 0) {
                            // Draw light grid on non-background pixels
                            out[idx * 4..idx * 4 + 4].copy_from_slice(&[
                                r.saturating_add(0x0F),
                                g.saturating_add(0x0F),
                                b.saturating_add(0x0F),
                                0xFF,
                            ]);
                        } else {
                            // Draw normal pixel
                            out[idx * 4..idx * 4 + 4].copy_from_slice(&[r, g, b, 0xFF]);
                        }
                    } else {
                        // GBC mode
                        if scale >= 2 && (dx == 0 || dy == 0) {
                            // Draw grid for colored LCD
                            out[idx * 4..idx * 4 + 4].copy_from_slice(&[
                                r.saturating_sub(r / 2),
                                g.saturating_sub(g / 2),
                                b.saturating_sub(b / 2),
                                0xFF,
                            ]);
                        } else {
                            // Draw normal pixel
                            out[idx * 4..idx * 4 + 4].copy_from_slice(&[r, g, b, 0xFF]);
                        }
                    }
                }
            }
        }
    }
}

pub fn crt(buffer: &LCD, out: &mut [u8], scale: usize) {
    for x in 0..(LCDW as u8) {
        for y in 0..(LCDH as u8) {
            let idx = LCD::to_idx(x, y, 1, 0, 0);
            let [_, r, g, b] = buffer.frame[idx].to_be_bytes();
            for dx in 0..scale {
                for dy in 0..scale {
                    let idx = LCD::to_idx(x, y, scale, dx, dy);
                    if scale >= 2 && dy == scale - 1 {
                        // Draw scanlines
                        out[idx * 4..idx * 4 + 4].copy_from_slice(&[r / 3, g / 3, b / 3, 0xFF]);
                    } else if scale >= 3 && dx == 0 {
                        // Draw left chromatic aberration
                        out[idx * 4..idx * 4 + 4].copy_from_slice(&[r.saturating_add(0x30), g, b, 0xFF]);
                    } else if scale >= 3 && dx == scale - 1 {
                        // Draw right chromatic aberration
                        out[idx * 4..idx * 4 + 4].copy_from_slice(&[r, g, b.saturating_add(0x30), 0xFF]);
                    } else {
                        // Draw normal pixel
                        out[idx * 4..idx * 4 + 4].copy_from_slice(&[r, g, b, 0xFF]);
                    }
                }
            }
        }
    }
}

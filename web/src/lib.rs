use std::{panic, rc::Rc};
use std::cell::RefCell;
use wasm_bindgen::{prelude::*, Clamped};
use web_sys::{console, window, CanvasRenderingContext2d, HtmlCanvasElement, ImageData, KeyboardEvent};
use base64::{engine::general_purpose, Engine as _};

use gb_core::{lcd, GBEmu, Joypad};

const SCALE: usize = 4;
const PALETTE_IDX_KEY: &str = "palette_idx";

struct EmuState {
    speed: u32,
    switch_palette: Option<bool>,
    switch_3d_mode: Option<bool>,
    rewind: bool,
    joypad: Joypad,
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window().unwrap().request_animation_frame(f.as_ref().unchecked_ref()).unwrap();
}

fn key_status_change(state: &mut EmuState, event: &KeyboardEvent, is_down: bool) {
    event.prevent_default();
    match event.code().as_str() {
        "KeyA" => state.joypad.a = is_down,
        "KeyS" => state.joypad.b = is_down,
        "ArrowUp" => state.joypad.up = is_down,
        "ArrowDown" => state.joypad.down = is_down,
        "ArrowLeft" => state.joypad.left = is_down,
        "ArrowRight" => state.joypad.right = is_down,
        "Enter" => state.joypad.start = is_down,
        "Backspace" => state.joypad.select = is_down,
        "Equal" if !is_down => state.speed = (state.speed * 2).clamp(1, 32),
        "Minus" if !is_down => state.speed = (state.speed / 2).clamp(1, 32),
        "Tab" if !is_down && !event.shift_key() => state.switch_palette = Some(true),
        "Tab" if !is_down && event.shift_key() => state.switch_palette = Some(false),
        "KeyP" if !is_down && !event.shift_key() => state.switch_3d_mode = Some(true),
        "KeyP" if !is_down && event.shift_key() => state.switch_3d_mode = Some(false),
        "KeyR" => state.rewind = is_down,
        _ => (),
    };
}


#[wasm_bindgen]
pub fn start(rom: &[u8]) {
    // Init emulator
    let mut emulator = GBEmu::new(&rom, false);
    let savekey = format!("{} - {}", emulator.rom_checksum(), emulator.rom_title());
    let (lcdw, lcdh) = (lcd::LCDW * SCALE, lcd::LCDH * SCALE);
    let state = Rc::new(RefCell::new(EmuState { speed: 1, switch_palette: None, switch_3d_mode: None, rewind: false, joypad: Joypad::default() }));

    // Init window and canvas
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    let document = web_sys::window().unwrap().document().unwrap();
    let local_storage = window().unwrap().local_storage().unwrap().unwrap();
    let canvas: HtmlCanvasElement = document.query_selector("canvas").unwrap().unwrap().dyn_into().unwrap();
    let context: CanvasRenderingContext2d = canvas.get_context("2d").unwrap().unwrap().dyn_into().unwrap();
    document.set_title(emulator.rom_title().as_str());
    canvas.set_width(lcdw as u32);
    canvas.set_height(lcdh as u32);

    // Init listener for key events
    let state_key_down = state.clone();
    let on_key_down = Closure::wrap(Box::new(move |event| { key_status_change(&mut state_key_down.borrow_mut(), &event, true) }) as Box<dyn FnMut(_)>);
    document.add_event_listener_with_callback("keydown", on_key_down.as_ref().unchecked_ref()).unwrap();
    on_key_down.forget();
    let state_key_up = state.clone();
    let on_key_up = Closure::wrap(Box::new(move |event| { key_status_change(&mut state_key_up.borrow_mut(), &event, false) }) as Box<dyn FnMut(_)>);
    document.add_event_listener_with_callback("keyup", on_key_up.as_ref().unchecked_ref()).unwrap();
    on_key_up.forget();

    // Load save file if present
    match local_storage.get_item(savekey.as_str()).unwrap() {
        Some(base64_save) => emulator.load_save(&general_purpose::STANDARD.decode(base64_save).unwrap()),
        None => console::log_1(&"Could not find save file".into()),
    }

    // Restore last used palette
    match local_storage.get_item(PALETTE_IDX_KEY).unwrap() {
        Some(palette_idx) => emulator.set_palette(palette_idx.parse().unwrap()),
        None => (),
    }

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    let mut frame_count = 0;
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let mut state = state.borrow_mut();
        let frame_buffer = loop {
            // Update palette
            if let Some(switch) = state.switch_palette.take() {
                let new_palette_idx = emulator.current_palette() + if switch { 1 } else { -1 };
                emulator.set_palette(new_palette_idx);
                local_storage.set_item(PALETTE_IDX_KEY, &new_palette_idx.to_string()).unwrap();
            }
            // Update 3D mode
            if let Some(switch) = state.switch_3d_mode.take() {
                let new_3d_mode_idx = emulator.current_3d_mode() + if switch { 1 } else { -1 };
                emulator.set_3d_mode(new_3d_mode_idx);
            }

            let frame_buffer = if state.rewind && emulator.can_rewind() {
                // Rewind state if requested
                emulator.rewind()
            } else {
                // Run emulator steps until a frame is available to be drawn
                emulator.step(&state.joypad)
            };

            // Return available frame
            if let Some(frame_buffer) = frame_buffer {
                frame_count += 1;
                // Skip drawn frames to match the requested speed
                if frame_count % state.speed == 0 {
                    break Some(frame_buffer)
                } 
            }
        };

        // Resize image to match scaled canvas
        let mut buffer = vec![0; lcdw * lcdh * 4];
        frame_buffer.unwrap().write_frame(&mut buffer, SCALE);

        // Convert to ImageData and push to canvas
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&buffer), lcdw as u32, lcdh as u32).unwrap();
        context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        // Save RAM content to file every 60 frames (~1s)
        if frame_count % 60 == 0 {
            let base64_save = general_purpose::STANDARD.encode(emulator.save());
            local_storage.set_item(&savekey, &base64_save).unwrap();
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

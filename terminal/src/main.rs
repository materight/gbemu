use ansi_colours::ansi256_from_rgb;
use clap::Parser;
use console_engine::{pixel, Color, ConsoleEngine};
use device_query::{DeviceQuery, DeviceState, Keycode};
use std::{fs, path::Path};

use gb_core::{lcd, GBEmu, Joypad};

#[derive(Parser)]
#[command(about = "A simple Gameboy emulator written in Rust")]
struct Args {
    /// ROM path (.gb/.gbc)
    #[arg(short, long)]
    file: String,

    /// Convert colors to ANSI value (8bpp), in case the terminal does not support true colors (24bpp)
    #[arg(long, action)]
    ansi: bool,

    /// Force games to run in DMG (Non-Color GB)
    #[arg(long, action)]
    force_dmg: bool,
}

fn main() {
    let args = Args::parse();

    // Read ROM and init emulator state
    let filepath = Path::new(&args.file);
    let rom = fs::read(filepath).expect("ROM not found");
    let mut emulator: GBEmu = GBEmu::new(&rom, args.force_dmg);

    // Load savefile if present
    let savepath = filepath.with_file_name(format!(".{}.sav", filepath.file_name().unwrap().to_string_lossy()));
    match fs::read(savepath.clone()) {
        Ok(savefile) => emulator.load_save(&savefile),
        Err(_) => println!("Could not find save file"),
    }

    // Setup output canvas
    let device_state = DeviceState::new();
    let mut engine = ConsoleEngine::init(lcd::LCDW as u32, lcd::LCDH as u32 / 2 + 1, 60).unwrap();
    engine.set_title(emulator.rom_title().as_str());
    let controls_help = "\
        [A] A    [S]: B    [↑↓←→] D-PAD    \
        [ENTER] START    [BACKSPACE] SELECT    \
        [TAB] SWITCH PALETTE    [R] REWIND    [ESC] EXIT\
    ";

    // Start emulation loop
    let mut running = true;
    let mut rewinding = false;
    let mut joypad = Joypad::default();
    let mut frame_count: u64 = 0;
    while running {
        // Run emulator step, i.e. execute next opcode
        let frame_buffer = if rewinding && emulator.can_rewind() {
            // Rewind to last state
            emulator.rewind()
        } else {
            // Run emulator step, i.e. execute next opcode
            emulator.step()
        };

        // Executed once per frame
        if let Some(frame_buffer) = frame_buffer {
            // Wait for next frame and capture inputs
            engine.wait_frame();
            frame_count += 1;

            // Draw frame to console buffer
            for x in 0..lcd::LCDW {
                for y in 0..lcd::LCDH / 2 {
                    let idxh = lcd::LCD::to_idx(x, y * 2, 1, 0, 0);
                    let idxl = lcd::LCD::to_idx(x, y * 2 + 1, 1, 0, 0);
                    let [rh, gh, bh, _] = frame_buffer.frame[idxh].to_be_bytes();
                    let [rl, gl, bl, _] = frame_buffer.frame[idxl].to_be_bytes();
                    let (bg_color, fg_color) = if !args.ansi {
                        (Color::Rgb { r: rh, g: gh, b: bh }, Color::Rgb { r: rl, g: gl, b: bl })
                    } else {
                        (
                            Color::AnsiValue(ansi256_from_rgb((rh, gh, bh))),
                            Color::AnsiValue(ansi256_from_rgb((rl, gl, bl))),
                        )
                    };
                    engine.set_pxl(x as i32, y as i32, pixel::pxl_fbg('▄', fg_color, bg_color));
                }
            }
            engine.print(0, lcd::LCDH as i32 / 2, controls_help);
            engine.draw();

            // Retrieve current pressed keys and update joypad
            let keys: Vec<Keycode> = device_state.get_keys();
            joypad.a = keys.contains(&Keycode::A);
            joypad.b = keys.contains(&Keycode::S);
            joypad.up = keys.contains(&Keycode::Up);
            joypad.down = keys.contains(&Keycode::Down);
            joypad.left = keys.contains(&Keycode::Left);
            joypad.right = keys.contains(&Keycode::Right);
            joypad.start = keys.contains(&Keycode::Enter);
            joypad.select = keys.contains(&Keycode::Backspace);
            emulator.set_joypad(&joypad);

            // Handle shortcuts
            rewinding = keys.contains(&Keycode::R);
            if engine.is_key_pressed(console_engine::KeyCode::Esc) {
                running = false;
            }
            if engine.is_key_pressed(console_engine::KeyCode::Tab) {
                emulator.set_palette(emulator.current_palette() + 1)
            }

            // Save RAM content to file every 60 frames (~1s)
            if frame_count % 60 == 0 {
                fs::write(savepath.clone(), emulator.save()).unwrap();
            }
        }
    }
}

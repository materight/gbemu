use std::{fs, path::Path};
use std::time::Duration;
use clap::Parser;
use gb_core::debug;
use minifb::{clamp, Key, KeyRepeat, Scale, Window, WindowOptions};

use gb_core::{GBEmu, Joypad, lcd};


#[derive(Parser)]
#[command(about = "A simple Gameboy emulator written in Rust")]
struct Args {
    /// ROM path (.gb/.gbc)
    #[arg(short, long)]
    file: String,

    /// Scale of the diplay
    #[arg(short, long, default_value_t = 4)]
    scale: u8,

    /// Force games to run in DMG (Non-Color GB)
    #[arg(long, action)]
    force_dmg: bool,

    /// Display loaded tiles
    #[arg(long, action)]
    tiles: bool,

    /// Print OP codes and registers
    #[arg(long, action)]
    debug: bool,
}

fn set_emulation_speed(window: &mut Window, speed: f32) {
    window.limit_update_rate(Some(Duration::from_micros((16742 as f32 / speed) as u64)));
}


fn main() {
    let args = Args::parse();
    debug::set_enabled(args.debug);
    let filepath = Path::new(&args.file);
    let rom = fs::read(filepath).expect("ROM not found");
    let mut emulator = GBEmu::new(&rom, args.force_dmg);
    let topmost = false;
    let scale = match args.scale {
        1 => Scale::X1, 2 => Scale::X2, 4 => Scale::X4, 8 => Scale::X8,
        _ => panic!("Unsupported scale: x{}", args.scale)
    };

    // Load savefile if present
    let savepath = filepath.with_file_name(format!(".{}.sav", filepath.file_name().unwrap().to_string_lossy()));
    match fs::read(savepath.clone()) {
        Ok(savefile) => emulator.load_save(&savefile),
        Err(_) => println!("Could not find save file")
    }

    // Setup tilemap window
    let mut tile_window = if args.tiles {
        Some(Window::new(
            "TILES",
            debug::TILEW, debug::TILEH,
            WindowOptions { scale: Scale::X2, topmost: topmost, ..Default::default() },
        ).unwrap())
    } else { None };

    // Setup output window
    let mut speed = 1.0;
    let mut paused = false;
    let mut window = Window::new(
        emulator.rom_title().as_str(),
        lcd::LCDW, lcd::LCDH,
        WindowOptions { scale: scale, topmost: topmost, ..Default::default() },
    ).unwrap();
    set_emulation_speed(&mut window, speed);

    // Start emulation loop
    let mut frame_count: u64 = 0;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        if paused {
            window.update();
            if window.is_key_pressed(Key::Space, KeyRepeat::No) { paused = false }
            else { continue }
        }

        // Retrieve current pressed keys 
        let joypad = Joypad {
            a: window.is_key_down(Key::A),
            b: window.is_key_down(Key::S),
            up: window.is_key_down(Key::Up),
            down: window.is_key_down(Key::Down),
            left: window.is_key_down(Key::Left),
            right: window.is_key_down(Key::Right),
            start: window.is_key_down(Key::Enter),
            select: window.is_key_down(Key::Backspace),
        };

        let frame_buffer = if window.is_key_down(Key::R) && emulator.can_rewind() {
            // Rewind to last state
            emulator.rewind()
        } else {
            // Run emulator step, i.e. execute next opcode
            emulator.step(&joypad)
        };

        // Executed once per frame
        if let Some(frame_buffer) = frame_buffer {
            frame_count += 1;

            // Write frame to buffer
            window.update_with_buffer(frame_buffer, lcd::LCDW, lcd::LCDH).unwrap();

            // Write tiles
            if let Some(wnd) = &mut tile_window {
                (*wnd).update_with_buffer(&emulator.draw_tilemap(), debug::TILEW, debug::TILEH).unwrap();
            }

            // Handle shortcuts
            if window.is_key_pressed(Key::Space, KeyRepeat::No) { paused = true }
            if window.is_key_released(Key::Equal) { speed *= 2.0 }
            if window.is_key_released(Key::Minus) { speed /= 2.0 }
            if window.is_key_released(Key::Tab) {
                let new_palette_idx = emulator.current_palette() + if !window.is_key_down(Key::LeftShift) { 1 } else { -1 }; 
                emulator.set_palette(new_palette_idx);
            }
            speed = clamp(1.0, speed, 256.0);
            set_emulation_speed(&mut window, speed);

            // Save RAM content to file every 60 frames (~1s)
            if frame_count % 60 == 0 {
                fs::write(savepath.clone(), emulator.save()).unwrap();
            }
        }
    }
}


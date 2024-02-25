use std::{fs, path::Path};
use std::time::Duration;
use clap::Parser;
use minifb::{clamp, Key, KeyRepeat, Menu, Scale, Window, WindowOptions};

use gb_core::{GBEmu, Joypad, lcd, palette};


#[derive(Parser)]
#[command(about = "A simple Gameboy emulator written in Rust")]
struct Args {
    #[arg(short, long)]
    file: String,

    #[arg(short, long, default_value_t = 4)]
    scale: u8,
}

fn set_emulation_speed(window: &mut Window, speed: f32) {
    window.limit_update_rate(Some(Duration::from_micros((16742 as f32 / speed) as u64)));
}


fn main() {
    let args = Args::parse();
    let filepath = Path::new(&args.file);
    let rom = fs::read(filepath).expect("ROM not found");
    let mut emulator = GBEmu::new(&rom);

    // Load savefile if present
    let savepath = filepath.with_file_name(format!(".{}.sav", filepath.file_stem().unwrap().to_string_lossy()));
    match fs::read(savepath.clone()) {
        Ok(savefile) => emulator.load_save(&savefile),
        Err(_) => println!("Could not find save file")
    }

    // Setup output window
    let scale = match args.scale {
         1 => Scale::X1, 2 => Scale::X2, 4 => Scale::X4, 8 => Scale::X8, _ => panic!("Unsupported scale: X{}", args.scale)
    };
    let mut speed = 8.0;
    let mut paused = false;
    let mut window = Window::new(
        emulator.rom_title().as_str(),
        lcd::LCDW, lcd::LCDH,
        WindowOptions {scale: scale, topmost: false, ..Default::default()},
    ).unwrap();
    set_emulation_speed(&mut window, speed);

    // Setup window menu with palette selection
    let mut menu = Menu::new("Settings").unwrap();
    let mut sub_palette = Menu::new("Palette").unwrap();
    for (i, (palette_name, _)) in palette::PALETTES.iter().enumerate() {
        sub_palette.add_item(palette_name, i).build();
    }
    menu.add_sub_menu("Palette", &sub_palette);
    window.add_menu(&menu);

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

        // Run emulator step, i.e. execute next opcode
        let new_frame_buffer = emulator.step(&joypad);

        // Executed once per frame
        if let Some(frame_buffer) = new_frame_buffer {
            frame_count += 1;

            // Write frame to buffer
            window.update_with_buffer(frame_buffer, lcd::LCDW, lcd::LCDH).unwrap();

            // Update palette
            if let Some(menu_id) = window.is_menu_pressed() {
                emulator.set_palette(menu_id);
            }

            // Handle shortcuts
            if window.is_key_pressed(Key::Space, KeyRepeat::No) { paused = true }
            if window.is_key_released(Key::Equal) { speed *= 2.0 }
            if window.is_key_released(Key::Minus) { speed /= 2.0 }
            speed = clamp(1.0, speed, 256.0);
            set_emulation_speed(&mut window, speed);

            // Save RAM content to file every 60 frames (~1s)
            if frame_count % 60 == 0 {
                fs::write(savepath.clone(), emulator.save()).unwrap();
            }
        }
    }
}


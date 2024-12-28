use std::{fs, path::Path};
use clap::Parser;
use gb_core::debug;
use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};

use gb_core::{GBEmu, Joypad, lcd};


#[derive(Parser)]
#[command(about = "A simple Gameboy emulator written in Rust")]
struct Args {
    /// ROM path (.gb/.gbc)
    #[arg(short, long)]
    file: String,

    /// Scale of the diplay
    #[arg(short, long, default_value_t = 4)]
    scale: u32,

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


fn main() {
    let args = Args::parse();
    debug::set_enabled(args.debug);
    let filepath = Path::new(&args.file);
    let rom = fs::read(filepath).expect("ROM not found");
    let mut emulator = GBEmu::new(&rom, args.force_dmg);

    // Load savefile if present
    let savepath = filepath.with_file_name(format!(".{}.sav", filepath.file_name().unwrap().to_string_lossy()));
    match fs::read(savepath.clone()) {
        Ok(savefile) => emulator.load_save(&savefile),
        Err(_) => println!("Could not find save file")
    }

    // Setup output window
    let (lcdw, lcdh) = (args.scale * lcd::LCDW as u32, args.scale * lcd::LCDH as u32);
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut canvas = video_subsystem
        .window(emulator.rom_title().as_str(), lcdw, lcdh)
        .position_centered()
        .opengl()
        .build()
        .unwrap()
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .unwrap();

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::ABGR8888, lcdw, lcdh)
        .unwrap();

    // Setup tilemap window
    let mut tile_canvas = video_subsystem
        .window("TILES", debug::TILEW as u32, debug::TILEH as u32)
        .opengl()
        .hidden()
        .build()
        .unwrap()
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .unwrap();

    let tile_texture_creator = tile_canvas.texture_creator();
    let mut tile_texture = tile_texture_creator
        .create_texture_streaming(PixelFormatEnum::ABGR8888, debug::TILEW as u32, debug::TILEH as u32)
        .unwrap();

    if args.tiles {
        tile_canvas.window_mut().show();
        canvas.window_mut().raise();
    }

    // Start emulation loop
    let mut running = true;
    let mut rewinding = false;
    let mut joypad = Joypad::default();
    let mut speed: u64 = 1;
    let mut frame_count: u64 = 0;
    while running {

        // Run emulator step, i.e. execute next opcode
        let frame_buffer = if rewinding && emulator.can_rewind() {
            // Rewind to last state
            emulator.rewind()
        } else {
            // Run emulator step, i.e. execute next opcode
            emulator.step(&joypad)
        };

        // Executed once per frame
        if let Some(frame_buffer) = frame_buffer {
            frame_count += 1;

            // Skip frames based on speed
            if frame_count % speed == 0 {
                // Write frame to buffer
                texture.with_lock(None, |buffer: &mut [u8], _| {
                    frame_buffer.write_frame(buffer, args.scale as usize)
                }).unwrap();
                canvas.copy(&texture, None, None).unwrap();
                canvas.present();

                // Write tiles
                if args.tiles {
                    let tilemap = emulator.draw_tilemap();
                    tile_texture.update(None, &tilemap, debug::TILEW * 4).unwrap();
                    tile_canvas.copy(&tile_texture, None, None).unwrap();
                    tile_canvas.present();
                }
            }

            // Handle key events
            for event in event_pump.poll_iter() {
                match event {
                    // Shortcuts
                    Event::Quit { .. } | Event::KeyUp { keycode: Some(Keycode::Escape), .. } => running = false,
                    Event::KeyDown { keycode: Some(Keycode::R), repeat: false, ..} => rewinding = true,
                    Event::KeyUp { keycode: Some(Keycode::R), repeat: false, ..} => rewinding = false,
                    Event::KeyUp { keycode: Some(Keycode::Equals), .. } if speed < 32 => speed *= 2,
                    Event::KeyUp { keycode: Some(Keycode::Minus), .. } if speed > 1 => speed /= 2,
                    Event::KeyUp { keycode: Some(Keycode::Tab), keymod: Mod::NOMOD, .. } => emulator.set_palette(emulator.current_palette() + 1),
                    Event::KeyUp { keycode: Some(Keycode::Tab), keymod: Mod::LSHIFTMOD, .. } => emulator.set_palette(emulator.current_palette() - 1),
                    Event::KeyUp { keycode: Some(Keycode::P), keymod: Mod::NOMOD, .. } => emulator.set_3d_mode(emulator.current_3d_mode() + 1),
                    Event::KeyUp { keycode: Some(Keycode::P), keymod: Mod::LSHIFTMOD, .. } => emulator.set_3d_mode(emulator.current_3d_mode() - 1),
                    // Joypad
                    Event::KeyDown { keycode: Some(Keycode::A), repeat: false, .. } => joypad.a = true,
                    Event::KeyUp { keycode: Some(Keycode::A), repeat: false, .. } => joypad.a = false,
                    Event::KeyDown { keycode: Some(Keycode::S), repeat: false, .. } => joypad.b = true,
                    Event::KeyUp { keycode: Some(Keycode::S), repeat: false, .. } => joypad.b = false,
                    Event::KeyDown { keycode: Some(Keycode::Up), repeat: false,.. } => joypad.up = true,
                    Event::KeyUp { keycode: Some(Keycode::Up), repeat: false,.. } => joypad.up = false,
                    Event::KeyDown { keycode: Some(Keycode::Down), repeat: false,.. } => joypad.down = true,
                    Event::KeyUp { keycode: Some(Keycode::Down), repeat: false,.. } => joypad.down = false,
                    Event::KeyDown { keycode: Some(Keycode::Left), repeat: false,.. } => joypad.left = true,
                    Event::KeyUp { keycode: Some(Keycode::Left), repeat: false,.. } => joypad.left = false,
                    Event::KeyDown { keycode: Some(Keycode::Right), repeat: false, .. } => joypad.right = true,
                    Event::KeyUp { keycode: Some(Keycode::Right), repeat: false, .. } => joypad.right = false,
                    Event::KeyDown { keycode: Some(Keycode::Return), repeat: false, .. } => joypad.start = true,
                    Event::KeyUp { keycode: Some(Keycode::Return), repeat: false, .. } => joypad.start = false,
                    Event::KeyDown { keycode: Some(Keycode::Backspace), repeat: false, .. } => joypad.select = true,
                    Event::KeyUp { keycode: Some(Keycode::Backspace), repeat: false, .. } => joypad.select = false,
                    _ => {}
                }
            }

            // Save RAM content to file every 60 frames (~1s)
            if frame_count % 60 == 0 {
                let save_data = emulator.save();
                fs::write(savepath.clone(), save_data).unwrap();
            }

        }
    }
}


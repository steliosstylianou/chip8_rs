mod chip8;
mod constants;
use crate::chip8::Chip8;
use crate::constants::*;

use clap::Parser;
use log::{debug, info};
use pixels::{Error, Pixels, SurfaceTexture};
use std::time::Instant;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    binary: String,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    let binary = args.binary;
    env_logger::init();
    info!("Starting Chip8 interpreter with binary {}", binary);
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(
            (CHIP8_WIDTH * CHIP8_WIN_SCALING_WIDTH) as f64,
            (CHIP8_HEIGHT * CHIP8_WIN_SCALING_HEIGHT) as f64,
        );
        WindowBuilder::new()
            .with_title("Chip8")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };
    let pixels = {
        let window_size = window.inner_size();
        let surface_texture: SurfaceTexture<'_, winit::window::Window> =
            SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(CHIP8_WIDTH as u32, CHIP8_HEIGHT as u32, surface_texture).unwrap()
    };

    let mut chip8 = Chip8::new(pixels, binary.as_str());

    event_loop.run(move |event, _, control_flow| {
        let start: Instant = Instant::now();

        control_flow.set_poll();
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    debug!("Exiting");
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                WindowEvent::Resized(size) => {
                    debug!("Resizing window...");
                    chip8.resize_window(size.width, size.height);
                    window.request_redraw();
                }
                WindowEvent::KeyboardInput { input, .. } => {
                    let scancode = input.virtual_keycode.expect("Invalid keycode.");
                    if chip8.keyboard_map.contains_key(&scancode) {
                        let key = chip8.keyboard_map[&scancode];
                        let previously_pressed = chip8.keypad[key as usize];
                        let just_pressed = input.state == ElementState::Pressed;
                        chip8.keypad[key as usize] = just_pressed;
                        chip8.key_pressed = if previously_pressed && !just_pressed {
                            Some(key)
                        } else {
                            None
                        };
                        debug!(
                            "Updating keypad {}({:?}) to {}",
                            key, scancode, just_pressed
                        );
                    }
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                debug!("Requested redraw");
                chip8.draw();
            }
            _ => (),
        }
        chip8.cycle();
        let fps = 1000.0 / (start.elapsed().as_millis() as f64);
        if fps < 80.0 {
            debug!("WTF");
        }
        debug!("Current FPS: {}", fps);
    });
}

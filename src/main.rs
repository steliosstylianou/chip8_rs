mod chip8;
mod constants;
mod sleep;

use crate::chip8::Chip8;
use crate::constants::*;
use crate::sleep::Sleeper;
use winit::event::VirtualKeyCode;

use clap::Parser;
use log::{debug, info};
use pixels::{Error, Pixels, SurfaceTexture};
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
    #[arg(long, default_value_t = 2)]
    scale: u32,
}



fn main() -> Result<(), Error> {
    env_logger::init();
    let args = Args::parse();
    let binary = args.binary.as_str();
    let scale = args.scale;
    info!(
        "Starting Chip8 interpreter with binary {} and scale {}",
        binary, scale
    );

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

    let mut chip8 = Chip8::new(pixels)
        .load_binary(binary)
        .unwrap_or_else(|_| panic!("Could not load binary {}", binary));

    let instruction_duty_cycle = chip8.time_per_insn();
    let mut sleeper = Sleeper::new(instruction_duty_cycle);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    info!("Exiting");
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
                    if scancode == VirtualKeyCode::Escape {
                        info!("Exiting");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
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
                        window.request_redraw();
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
        chip8.step();
        sleeper.sleep();
    });
}

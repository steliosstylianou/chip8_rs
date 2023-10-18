use log::{debug, trace};
use pixels::Pixels;
use rand::Rng;
use rodio::source::SineWave;
use rodio::{OutputStream, Sink};
use std::collections::HashMap;
use std::time::Instant;
use winit::event::VirtualKeyCode;

pub mod constants;
use constants::*;
mod sleeper;
use sleeper::Sleeper;

fn nibbles(insn: u16) -> (u8, u8, u8, u8) {
    (
        ((insn >> 12) & 0xF) as u8,
        ((insn >> 8) & 0xF) as u8,
        ((insn >> 4) & 0xF) as u8,
        (insn & 0xF) as u8,
    )
}

pub struct Interpreter {
    v: [u8; 16],
    i: u16,
    delay_timer: u8,
    sound_timer: u8,
    pc: u16,
    sp: u8,
    stack: [u16; 16],
    pixels: Pixels,
    memory: [u8; CHIP8_MEMORY_SIZE],
    pub keyboard_map: HashMap<VirtualKeyCode, u32>,
    pub keypad: [bool; 16],
    pub key_pressed: Option<u32>,
    pub cycle_count: u32,
    hz: u32,
    timer: Instant,
    sleeper: Sleeper,
    _sound_stream: OutputStream,
    sound_sink: Sink,
}

type Reg = u8;
type Addr = u16;

#[derive(Debug)]
pub enum Instruction {
    Nop,
    Clear,
    Return,
    Jump(Addr),
    Call(Addr),
    LoadI(Addr),
    JumpOff(Addr),
    AddI(Reg),
    LoadRegs(Reg),
    StoreRegs(Reg),
    StoreBcd(Reg),
    SetSpriteAddr(Reg),
    SkipPressed(Reg),
    SkipNotPressed(Reg),
    WaitKeypress(Reg),
    LoadFromDelayTimer(Reg),
    LoadDelayTimer(Reg),
    LoadSoundTimer(Reg),
    Shl(Reg),
    Shr(Reg),
    SkipEq(Reg, Reg),
    SkipEqIm(Reg, u8),
    SkipNe(Reg, Reg),
    SkipNeIm(Reg, u8),
    LoadIm(Reg, u8),
    AddIm(Reg, u8),
    Move(Reg, Reg),
    Or(Reg, Reg),
    And(Reg, Reg),
    Xor(Reg, Reg),
    Add(Reg, Reg),
    Sub(Reg, Reg),
    SubN(Reg, Reg),
    Rnd(Reg, u8),
    Draw(Reg, Reg, u8),
}

use Instruction::*;

impl Interpreter {
    pub fn new(pixels: Pixels) -> Interpreter {
        let keyboard_map = HashMap::from(CHIP8_KEYBOARD_MAP);
        let (stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        let source = SineWave::new(CHIP8_BEEP_FREQUENCY);
        sink.append(source);
        sink.pause();

        let mut chip = Interpreter {
            v: [0; 16],
            i: 0,
            delay_timer: 0,
            sound_timer: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16],
            memory: [0; CHIP8_MEMORY_SIZE],
            pixels,
            keyboard_map,
            keypad: [false; 16],
            key_pressed: None,
            cycle_count: 0,
            hz: CHIP8_SPEED_HZ,
            timer: Instant::now(),
            sleeper: Sleeper::new().with_frequency(CHIP8_SPEED_HZ),
            sound_sink: sink,
            _sound_stream: stream,
        };
        chip.load_fonts();
        chip
    }

    fn load_fonts(&mut self) {
        let fonts = CHIP8_FONT;
        self.memory[0..fonts.len()].copy_from_slice(&fonts);
    }

    pub fn draw(&self) {
        self.pixels.render().expect("Error while rendering pixels");
    }

    pub fn resize_window(&mut self, width: u32, height: u32) {
        self.pixels
            .resize_surface(width, height)
            .expect("Could not resize window");
    }

    pub fn load_binary(mut self, binary: &str) -> std::io::Result<Self> {
        debug!("Loading binary {binary}.");
        let buffer = std::fs::read(binary)?;
        let start_address = 0x200;
        self.memory[start_address..(start_address + buffer.len())].copy_from_slice(&buffer[..]);
        Ok(self)
    }

    fn fetch(&mut self) -> u16 {
        let pc = self.pc as usize;
        let instruction: u16 = u16::from_be_bytes([self.memory[pc], self.memory[pc + 1]]);
        trace!(
            "Fetched instruction {:#06x} from address {}.",
            instruction,
            self.pc,
        );
        self.pc += 2;
        instruction
    }

    fn decode(&self, instruction: u16) -> Instruction {
        let decoded_insn = match nibbles(instruction) {
            (0, 0, 0xE, 0) => Clear,
            (0, 0, 0xE, 0xE) => Return,
            (1, _, _, _) => {
                let nnn = instruction & 0xFFF;
                Jump(nnn)
            }
            (2, _, _, _) => {
                let nnn = instruction & 0xFFF;
                Call(nnn)
            }
            (3, x, _, _) => {
                let kk: u8 = (instruction & 0xFF) as u8;
                SkipEqIm(x, kk)
            }
            (4, x, _, _) => {
                let kk: u8 = (instruction & 0xFF) as u8;
                SkipNeIm(x, kk)
            }
            (5, x, y, 0) => SkipEq(x, y),
            (6, x, _, _) => {
                let kk: u8 = (instruction & 0xFF) as u8;
                LoadIm(x, kk)
            }
            (7, x, _, _) => {
                let kk: u8 = (instruction & 0xFF) as u8;
                AddIm(x, kk)
            }
            (8, x, y, 0) => Move(x, y),
            (8, x, y, 1) => Or(x, y),
            (8, x, y, 2) => And(x, y),
            (8, x, y, 3) => Xor(x, y),
            (8, x, y, 4) => Add(x, y),
            (8, x, y, 5) => Sub(x, y),
            (8, x, _, 6) => Shr(x),
            (8, x, y, 7) => SubN(x, y),
            (8, x, _, 0xE) => Shl(x),
            (9, x, y, 0) => SkipNe(x, y),
            (0xA, _, _, _) => {
                let nnn = instruction & 0xFFF;
                LoadI(nnn)
            }
            (0xB, _, _, _) => {
                let nnn = instruction & 0xFFF;
                JumpOff(nnn)
            }
            (0xC, x, _, _) => {
                let kk: u8 = (instruction & 0xFF) as u8;
                Rnd(x, kk)
            }
            (0xD, x, y, n) => Draw(x, y, n),
            (0xE, x, 9, 0xE) => SkipPressed(x),
            (0xE, x, 0xA, 1) => SkipNotPressed(x),
            (0xF, x, 0, 7) => LoadFromDelayTimer(x),
            (0xF, x, 0, 0xA) => WaitKeypress(x),
            (0xF, x, 1, 5) => LoadDelayTimer(x),
            (0xF, x, 1, 8) => LoadSoundTimer(x),
            (0xF, x, 1, 0xE) => AddI(x),
            (0xF, x, 2, 9) => SetSpriteAddr(x),
            (0xF, x, 3, 3) => StoreBcd(x),
            (0xF, x, 5, 5) => StoreRegs(x),
            (0xF, x, 6, 5) => LoadRegs(x),
            (_, _, _, _) => Nop,
        };
        trace!("Decoded instruction {:?}", decoded_insn);
        decoded_insn
    }

    fn execute(&mut self, insn: Instruction) {
        match insn {
            Nop => (),

            Clear => {
                let frame = self.pixels.frame_mut();
                for pixel in frame.chunks_exact_mut(4) {
                    pixel[0] = 0x00; // R
                    pixel[1] = 0x00; // G
                    pixel[2] = 0x00; // B
                    pixel[3] = 0xff; // A
                }
            }

            Return => {
                self.sp -= 1;
                let address = self.stack[self.sp as usize];
                self.pc = address;
            }

            Jump(addr) => {
                self.pc = addr;
            }

            Call(addr) => {
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = addr;
            }

            LoadI(addr) => {
                self.i = addr;
            }

            JumpOff(addr) => {
                self.pc = self.v[0] as u16 + addr;
            }

            AddI(reg) => {
                self.i += u16::from(self.v[reg as usize]);
            }

            LoadRegs(reg) => {
                let last_index = reg as usize;
                for i in 0..=last_index {
                    self.v[i] = self.memory[self.i as usize + i]
                }
            }

            StoreRegs(reg) => {
                let last_index = reg as usize;
                for i in 0..=last_index {
                    self.memory[self.i as usize + i] = self.v[i];
                }
            }

            StoreBcd(reg) => {
                let mut value = self.v[reg as usize];
                for i in (0..=2).rev() {
                    self.memory[(self.i + i) as usize] = value % 10;
                    value /= 10;
                }
            }

            SetSpriteAddr(reg) => {
                let digit = self.v[reg as usize];
                self.i = (digit as u16) * 5;
            }

            SkipPressed(reg) => {
                let key = self.v[reg as usize] as usize;
                if self.keypad[key] {
                    self.pc += 2;
                }
            }

            SkipNotPressed(reg) => {
                let key = self.v[reg as usize] as usize;
                if !self.keypad[key] {
                    self.pc += 2;
                }
            }

            WaitKeypress(reg) => {
                if let Some(key) = self.key_pressed {
                    self.keypad[key as usize] = false;
                    self.v[reg as usize] = key as u8;
                } else {
                    self.pc -= 2;
                }
            }

            LoadFromDelayTimer(reg) => {
                self.v[reg as usize] = self.delay_timer;
            }

            LoadDelayTimer(reg) => {
                let value = self.v[reg as usize];
                self.delay_timer = value;
            }

            LoadSoundTimer(reg) => {
                let value = self.v[reg as usize];
                self.sound_timer = value;
            }

            Shl(src_dst) => {
                let src_dst = src_dst as usize;
                let vf = (self.v[src_dst] >> 7) & 0x1;
                self.v[src_dst] <<= 1;
                self.v[0xf] = vf;
            }

            Shr(src_dst) => {
                let src_dst = src_dst as usize;
                let vf = self.v[src_dst] & 0x1;
                self.v[src_dst] >>= 1;
                self.v[0xf] = vf;
            }

            SkipEq(reg0, reg1) => {
                if self.v[reg0 as usize] == self.v[reg1 as usize] {
                    self.pc += 2;
                }
            }

            SkipEqIm(reg, value) => {
                if self.v[reg as usize] == value {
                    self.pc += 2;
                }
            }

            SkipNe(reg0, reg1) => {
                if self.v[reg0 as usize] != self.v[reg1 as usize] {
                    self.pc += 2;
                }
            }

            SkipNeIm(reg, value) => {
                if self.v[reg as usize] != value {
                    self.pc += 2;
                }
            }

            LoadIm(reg, value) => {
                self.v[reg as usize] = value;
            }

            AddIm(reg, value) => {
                self.v[reg as usize] = self.v[reg as usize].wrapping_add(value);
            }

            Move(dst, src) => {
                self.v[dst as usize] = self.v[src as usize];
            }

            Or(src_dst, src) => {
                self.v[src_dst as usize] |= self.v[src as usize];
            }

            And(src_dst, src) => {
                self.v[src_dst as usize] &= self.v[src as usize];
            }

            Xor(src_dst, src) => {
                self.v[src_dst as usize] ^= self.v[src as usize];
            }

            Add(src_dst, src) => {
                let (result, overflow) =
                    self.v[src_dst as usize].overflowing_add(self.v[src as usize]);
                self.v[src_dst as usize] = result;
                self.v[0xf] = if overflow { 1 } else { 0 };
            }

            Sub(src_dst, src) => {
                let (result, overflow) =
                    self.v[src_dst as usize].overflowing_sub(self.v[src as usize]);
                self.v[src_dst as usize] = result;
                self.v[0xf] = if overflow { 0 } else { 1 };
            }

            SubN(src_dst, src) => {
                let (result, overflow) =
                    self.v[src as usize].overflowing_sub(self.v[src_dst as usize]);
                self.v[src_dst as usize] = result;
                self.v[0xf] = if overflow { 0 } else { 1 };
            }

            Rnd(reg, value) => {
                let random_num: u8 = rand::thread_rng().gen_range(0..=255);
                self.v[reg as usize] = random_num & value;
            }

            Draw(x, y, no_lines) => {
                let x: usize = self.v[x as usize] as usize;
                let y: usize = self.v[y as usize] as usize;
                let no_lines: usize = no_lines.into();

                let fb = self.pixels.frame_mut();
                let sprite = &self.memory[self.i as usize..(self.i + no_lines as u16) as usize];

                // sprites are always 8-bit wide
                let sprite_len = 8;
                for (j, line) in sprite.iter().enumerate().take(no_lines) {
                    let yoff = ((y + j) % CHIP8_HEIGHT) * CHIP8_WIDTH;
                    for i in 0..sprite_len {
                        let xoff = (x + i) % CHIP8_WIDTH;
                        let pixel_coord = (xoff + yoff) * 4;
                        // fb format is RGBA, so convert it to monochrome (0->0, 255->1)
                        let old_value = if fb[pixel_coord] == 255 { 1 } else { 0 };
                        let sprite_value = (line >> (sprite_len - 1 - i)) & 0x1;
                        let mut new_val = sprite_value ^ old_value;
                        // Convert back to RGBA
                        if new_val == 1 {
                            new_val = 255;
                        }
                        fb[pixel_coord] = new_val; // R
                        fb[pixel_coord + 1] = new_val; // G
                        fb[pixel_coord + 2] = new_val; // B
                        fb[pixel_coord + 3] = 255; // A
                                                   // Detect collisions. Happens when both values are set
                        if old_value == 1 && sprite_value == 1 {
                            self.v[0xf] = 1;
                        }
                    }
                }
                self.pixels.render().expect("Error while rendering");
            }
        }
        // Clear any key-presses
        self.key_pressed = None;
        trace!("Executed instruction {:?}", insn);
        self.cycle_count = self.cycle_count.wrapping_add(1);
    }

    pub fn step(&mut self) {
        let current_insn = self.fetch();
        let decoded_insn: Instruction = self.decode(current_insn);
        self.execute(decoded_insn);
        self.update_timers();
        self.beep();
        self.sleep();
        self.print_ops();
    }

    fn beep(&self) {
        if self.sound_timer > 0 {
            self.sound_sink.play();
        } else {
            self.sound_sink.pause();
        }
    }

    fn sleep(&mut self) {
        self.sleeper.sleep();
    }

    fn update_timers(&mut self) {
        // Counters are updated at a frequency of 1/60th second.
        if self.cycle_count % (self.hz / 60) == 0 {
            self.delay_timer = self.delay_timer.saturating_sub(1);
            self.sound_timer = self.sound_timer.saturating_sub(1);
        }
    }

    fn print_ops(&mut self) {
        if (self.cycle_count % IPS_MEASURE_CYCLE) == 0 {
            // Divide by ms instead of s to get more accuracy so multiply by 1000.
            let ips = 1000 * IPS_MEASURE_CYCLE as u128 / self.timer.elapsed().as_millis();
            debug!("OPS: {}. Cycle count: {}", ips, self.cycle_count);
            self.timer = Instant::now();
        }
    }
}

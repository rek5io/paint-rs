use crate::brush_modes;
use crate::util::Key;
use crate::window::WindowState;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Default, Debug)]
pub enum BrushMode {
    #[default]
    NormalBrush,
    Fill,
    Line,
    Square,
    Triangle,
    Circle,
}

#[derive(Default, PartialEq, Eq)]
pub enum ControlStep {
    #[default]
    ClickFirst,
    ClickSecond,
}

pub struct Paint {
    pub background: Pixel,
    pub buffer_size: (u32, u32),
    pub screen_buffer: Vec<Pixel>,
    pub main_buffer: Vec<Pixel>,
    pub screens_buffer: VecDeque<((u32, u32), Vec<Pixel>)>,
    pub window: WindowState,
    pub brush_size: u32,
    pub brush_mode: BrushMode,
    pub color: Pixel,
    pub last_mouse_pos: (u32, u32),
    pub control_step: ControlStep,
}

impl Paint {
    pub fn new(width: u32, height: u32) -> Self {
        let bg = Pixel {
            r: 0x18,
            g: 0x18,
            b: 0x25,
        };

        Self {
            background: bg,
            buffer_size: (width, height),
            main_buffer: vec![bg; (width * height) as usize],
            screen_buffer: vec![bg; (width * height) as usize],
            screens_buffer: VecDeque::new(),
            window: WindowState::new(width, height),
            brush_size: 2,
            brush_mode: BrushMode::default(),
            color: Pixel {
                r: 0xcd,
                g: 0xd6,
                b: 0xf4,
            },
            last_mouse_pos: (0, 0),
            control_step: ControlStep::default(),
        }
    }

    pub fn run(&mut self) {
        while !self.window.want_close {
            self.window.dispatch();

            if self.last_mouse_pos == (0, 0) {
                self.last_mouse_pos = self.window.mouse;
            }

            if !self.window.keys_pressed.is_empty() {
                self.react_keys();
            }

            if self.window.clicked_left {
                if self.screens_buffer.len() >= 192 {
                    let _ = self.screens_buffer.pop_front();
                }

                self.screens_buffer
                    .push_back((self.window.size, self.main_buffer.clone()));

                println!("saved step: {}", self.screens_buffer.len());
            }

            (|| {
                let (new_w, new_h) = self.window.size;
                let (old_w, old_h) = self.buffer_size;
                if new_w <= old_w && new_h <= old_h {
                    return;
                }

                println!("resize up");

                let target_w = old_w.max(new_w);
                let target_h = old_h.max(new_h);

                let mut new_buf = vec![self.background; (target_w * target_h) as usize];

                for y in 0..old_h {
                    let old_row = (y * old_w) as usize;
                    let new_row = (y * target_w) as usize;

                    new_buf[new_row..new_row + old_w as usize]
                        .copy_from_slice(&self.main_buffer[old_row..old_row + old_w as usize]);
                }

                self.main_buffer = new_buf;
                self.screen_buffer = self.main_buffer.clone();
                self.buffer_size = (target_w, target_h);
            })();

            self.render_ui();

            self.window
                .put_frame(&self.screen_buffer, self.buffer_size.0, self.buffer_size.1);

            match self.brush_mode {
                BrushMode::NormalBrush => brush_modes::normal_brush_mode(self),
                BrushMode::Fill => brush_modes::fill_mode(self),
                _ => brush_modes::multi_mode(self),
            }
        }
    }
    pub fn react_keys(&mut self) {
        let keys = self.window.keys_pressed.clone();

        for key in keys {
            match key {
                Key::Esc => self.window.want_close = true,

                Key::Up => self.brush_size += 1,
                Key::Down => {
                    if self.brush_size > 1 {
                        self.brush_size -= 1;
                    }
                }

                Key::W => {
                    self.color = Pixel {
                        r: 0xcd,
                        g: 0xd6,
                        b: 0xf4,
                    };
                }

                Key::R => {
                    self.color = Pixel {
                        r: 0xf3,
                        g: 0x8b,
                        b: 0xa8,
                    };
                }

                Key::G => {
                    self.color = Pixel {
                        r: 0xa6,
                        g: 0xe3,
                        b: 0xa1,
                    };
                }

                Key::B => {
                    self.color = Pixel {
                        r: 0x89,
                        g: 0xb4,
                        b: 0xfa,
                    };
                }

                Key::P => {
                    self.color = Pixel {
                        r: 0xcb,
                        g: 0xa6,
                        b: 0xd7,
                    };
                }

                Key::Y => {
                    self.color = Pixel {
                        r: 0xf9,
                        g: 0xe2,
                        b: 0xaf,
                    };
                }

                Key::K => self.color = self.background,

                Key::F => self.brush_mode = BrushMode::Fill,
                Key::L => self.brush_mode = BrushMode::Line,
                Key::S => self.brush_mode = BrushMode::Square,
                Key::T => self.brush_mode = BrushMode::Triangle,
                Key::C => self.brush_mode = BrushMode::Circle,
                Key::N => self.brush_mode = BrushMode::NormalBrush,

                Key::Z => self.restore_step(),

                Key::LShift => {}
            }

            match key {
                Key::F | Key::L | Key::S | Key::T | Key::C | Key::N => {
                    self.control_step = ControlStep::ClickFirst;
                    self.screen_buffer = self.main_buffer.clone();
                }
                _ => {}
            }
        }
    }

    fn restore_step(&mut self) {
        if let Some((s, buffer)) = self.screens_buffer.pop_back() {
            println!("restored step: {}", self.screens_buffer.len());

            self.main_buffer = buffer.clone();
            self.screen_buffer = buffer;
            self.buffer_size = s;
        }
    }

    fn render_ui(&mut self) {
        for x in 0..=35 {
            for y in 0..=22 {
                let idx = ((self.buffer_size.1 - 20 - y) * self.buffer_size.0 + 20 + x) as usize;
                let Some(buffer) = self.screen_buffer.get_mut(idx) else {
                    continue;
                };

                *buffer = self.color;
            }
        }
    }
}

use std::{fs::File, os::unix::io::AsFd};

use memmap2::RemapOptions;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
};

use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::{paint::Pixel, util::Key};

pub struct WindowState {
    pub want_close: bool,
    base_surface: Option<wl_surface::WlSurface>,
    buffer: Option<wl_buffer::WlBuffer>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    xdg_surface: Option<(xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel)>,
    shm: Option<wl_shm::WlShm>,
    pool: Option<wl_shm_pool::WlShmPool>,
    ev: Option<EventQueue<WindowState>>,
    mmap: Option<memmap2::MmapMut>,
    file: Option<File>,
    configured: bool,
    pub keys_pressed: Vec<Key>,
    pub pressed_left: bool,
    pub clicked_left: bool,
    pub size: (u32, u32),
    pub mouse: (u32, u32),
    pending_size: Option<(u32, u32)>,
}

impl WindowState {
    pub fn new(width: u32, height: u32) -> Self {
        let conn = Connection::connect_to_env().unwrap();
        let ev: EventQueue<WindowState> = conn.new_event_queue();
        let qhandle = ev.handle();
        let display = conn.display();
        display.get_registry(&qhandle, ());

        let state = WindowState {
            want_close: false,
            base_surface: None,
            buffer: None,
            wm_base: None,
            xdg_surface: None,
            shm: None,
            pool: None,
            ev: Some(ev),
            mmap: None,
            file: None,
            configured: false,
            keys_pressed: Vec::with_capacity(64),
            pressed_left: false,
            clicked_left: false,
            size: (width, height),
            mouse: (1, 1),
            pending_size: None,
        };

        state
    }

    pub fn dispatch(&mut self) {
        self.clicked_left = false;
        let mut ev = self.ev.take().unwrap();
        ev.blocking_dispatch(self).unwrap();
        self.ev = Some(ev);
    }

    fn init_xdg_surface(&mut self, qh: &QueueHandle<WindowState>) {
        let wm_base = self.wm_base.as_ref().unwrap();
        let base_surface = self.base_surface.as_ref().unwrap();

        let xdg_surface = wm_base.get_xdg_surface(base_surface, qh, ());
        let toplevel = xdg_surface.get_toplevel(qh, ());
        toplevel.set_title("A fantastic window!".into());

        base_surface.commit();

        self.xdg_surface = Some((xdg_surface, toplevel));
    }

    fn show_surface(&mut self) {
        if let Some(ref surface) = self.base_surface
            && self.configured
        {
            surface.attach(self.buffer.as_ref(), 0, 0);
            surface.damage(0, 0, self.size.0 as i32, self.size.1 as i32);
            surface.commit();
        }
    }

    pub fn put_frame(&mut self, frame: &[Pixel], width: u32, height: u32) {
        let (win_w, win_h) = self.size;
        let copy_w = width.min(win_w);
        let copy_h = height.min(win_h);

        if let Some(buf) = self.mmap.as_mut() {
            for y in 0..copy_h {
                let src_row = (y * width) as usize;
                let dst_row = (y * win_w) as usize * 4;

                for x in 0..copy_w {
                    let pixel = &frame[src_row + x as usize];
                    let off = dst_row + x as usize * 4;
                    buf[off + 0] = pixel.b;
                    buf[off + 1] = pixel.g;
                    buf[off + 2] = pixel.r;
                    buf[off + 3] = 0xff;
                }
            }

            self.show_surface();
        }
    }

    fn recreate_buffer(&mut self, qh: &QueueHandle<WindowState>) {
        let shm = self.shm.as_ref().unwrap();
        let w = self.size.0;
        let h = self.size.1;
        let size = w * h * 4;

        if self.file.is_none() {
            self.file = Some(
                memfd::MemfdOptions::default()
                    .create("memfd_wl_window")
                    .unwrap()
                    .into_file(),
            );
        }

        let file = self.file.as_mut().unwrap();
        file.set_len(size as u64).unwrap();

        if let Some(mmap) = self.mmap.as_mut() {
            unsafe {
                mmap.remap(size as usize, RemapOptions::new().may_move(true))
                    .unwrap();
            }
        } else {
            let mmap = unsafe { memmap2::Mmap::map(file as &File).unwrap() };
            self.mmap = Some(mmap.make_mut().unwrap());
        }

        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());

        let buffer = pool.create_buffer(
            0,
            w as i32,
            h as i32,
            (w * 4) as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        if let Some(buffer) = self.buffer.take() {
            buffer.destroy();
        }

        if let Some(pool) = self.pool.take() {
            pool.destroy();
        }

        self.pool = Some(pool);
        self.buffer = Some(buffer);
        self.show_surface();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WindowState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    let compositor =
                        registry.bind::<wl_compositor::WlCompositor, _, _>(name, 1, qh, ());
                    let surface = compositor.create_surface(qh, ());
                    state.base_surface = Some(surface);

                    if state.wm_base.is_some() && state.xdg_surface.is_none() {
                        state.init_xdg_surface(qh);
                    }
                }

                "wl_shm" => {
                    let shm = registry.bind::<wl_shm::WlShm, _, _>(name, 1, qh, ());
                    state.shm = Some(shm);
                    state.recreate_buffer(qh);
                }

                "wl_seat" => {
                    registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ());
                }

                "xdg_wm_base" => {
                    let wm_base = registry.bind::<xdg_wm_base::XdgWmBase, _, _>(name, 1, qh, ());
                    state.wm_base = Some(wm_base);

                    if state.base_surface.is_some() && state.xdg_surface.is_none() {
                        state.init_xdg_surface(qh);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for WindowState {
    fn event(
        _state: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: <xdg_wm_base::XdgWmBase as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for WindowState {
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: <xdg_surface::XdgSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.configured = true;

            if let Some((w, h)) = state.pending_size.take() {
                if w != state.size.0 || h != state.size.1 {
                    state.size.0 = w;
                    state.size.1 = h;
                    state.recreate_buffer(qh);
                }
            }
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for WindowState {
    fn event(
        state: &mut WindowState,
        _: &xdg_toplevel::XdgToplevel,
        event: <xdg_toplevel::XdgToplevel as wayland_client::Proxy>::Event,
        _data: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0 && height > 0 {
                    state.pending_size = Some((width as u32, height as u32));
                }
            }

            xdg_toplevel::Event::Close => {
                state.want_close = true;
            }

            _ => {}
        }
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for WindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: <wl_compositor::WlCompositor as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for WindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: <wl_surface::WlSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for WindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: <wl_shm::WlShm as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for WindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: <wl_shm_pool::WlShmPool as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for WindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: <wl_buffer::WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WindowState {
    fn event(
        _state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            if let WEnum::Value(v) = capabilities {
                if v.contains(wl_seat::Capability::Keyboard) {
                    seat.get_keyboard(qh, ());
                }

                if v.contains(wl_seat::Capability::Pointer) {
                    seat.get_pointer(qh, ());
                }
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for WindowState {
    fn event(
        window_state: &mut Self,
        _proxy: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Key { key, state, .. } = event {
            match (state, Key::from_code(key)) {
                (WEnum::Value(wl_keyboard::KeyState::Pressed), Some(k)) => {
                    if !window_state.keys_pressed.contains(&k) {
                        window_state.keys_pressed.push(k);
                    }
                }
                (WEnum::Value(wl_keyboard::KeyState::Released), Some(k)) => {
                    window_state.keys_pressed.retain(|e| *e != k);
                }
                (_, None) => println!("unknown key: {}", key),
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for WindowState {
    fn event(
        s: &mut Self,
        _proxy: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        const KEY_LEFT: u32 = 272;

        match event {
            wl_pointer::Event::Button { button, state, .. } => {
                if button == KEY_LEFT {
                    if state == WEnum::Value(wl_pointer::ButtonState::Pressed) {
                        s.pressed_left = true;
                        s.clicked_left = true;
                    } else {
                        s.pressed_left = false;
                    }
                }
            }

            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                s.mouse = (
                    surface_x.clamp(0.0, u32::MAX as f64) as u32,
                    surface_y.clamp(0.0, u32::MAX as f64) as u32,
                );
            }

            _ => {}
        }
    }
}

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow, bail};
use softbuffer::{Context as SoftbufferContext, Surface as SoftbufferSurface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::ffi::DecodedFrame;

const DEFAULT_WINDOW_TITLE: &str = "Hikvision Thermal Viewer (Wayland)";
const DEFAULT_WINDOW_WIDTH: u32 = 960;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;

type WindowSoftbufferContext = SoftbufferContext<Arc<Window>>;
type WindowSoftbufferSurface = SoftbufferSurface<Arc<Window>, Arc<Window>>;

struct ViewerApp {
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: Arc<AtomicBool>,
    window: Option<Arc<Window>>,
    context: Option<WindowSoftbufferContext>,
    surface: Option<WindowSoftbufferSurface>,
    last_seq: u64,
    last_frame: Option<DecodedFrame>,
    surface_size: (u32, u32),
    blank: Vec<u32>,
    fatal_error: Option<String>,
}

impl ViewerApp {
    fn new(latest_frame: Arc<Mutex<Option<DecodedFrame>>>, stop_flag: Arc<AtomicBool>) -> Self {
        let blank_len = (DEFAULT_WINDOW_WIDTH.max(1) * DEFAULT_WINDOW_HEIGHT.max(1)) as usize;
        Self {
            latest_frame,
            stop_flag,
            window: None,
            context: None,
            surface: None,
            last_seq: 0,
            last_frame: None,
            surface_size: (0, 0),
            blank: vec![0xFF10_1010_u32; blank_len],
            fatal_error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, message: impl Into<String>) {
        let msg = message.into();
        eprintln!("{msg}");
        self.fatal_error = Some(msg);
        self.stop_flag.store(true, Ordering::Relaxed);
        event_loop.exit();
    }

    fn ensure_blank_len(&mut self, len: usize) {
        if self.blank.len() != len {
            self.blank.resize(len, 0xFF10_1010);
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.as_ref().map(Arc::clone) else {
            return;
        };
        if self.surface.is_none() {
            return;
        }

        if let Ok(guard) = self.latest_frame.lock()
            && let Some(frame) = guard.as_ref()
            && frame.sequence != self.last_seq
        {
            self.last_seq = frame.sequence;
            self.last_frame = Some(frame.clone());
        }

        let (w, h) = if let Some(frame) = self.last_frame.as_ref() {
            (frame.width, frame.height)
        } else {
            (DEFAULT_WINDOW_WIDTH.max(1), DEFAULT_WINDOW_HEIGHT.max(1))
        };

        if w == 0 || h == 0 {
            return;
        }

        let target_len = (w * h) as usize;
        self.ensure_blank_len(target_len);

        let mut surface = self
            .surface
            .take()
            .expect("surface was checked to be Some above");

        let draw_result = (|| -> std::result::Result<(), String> {
            if self.surface_size != (w, h)
                && let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h))
            {
                surface
                    .resize(nw, nh)
                    .map_err(|err| format!("softbuffer resize failed: {err}"))?;
                let _ = window.request_inner_size(LogicalSize::new(w as f64, h as f64));
                self.surface_size = (w, h);
            }

            let mut buffer = surface
                .buffer_mut()
                .map_err(|err| format!("softbuffer buffer_mut failed: {err}"))?;

            if let Some(frame) = self
                .last_frame
                .as_ref()
                .filter(|f| f.pixels_argb.len() == target_len)
            {
                if buffer.len() == frame.pixels_argb.len() {
                    buffer.copy_from_slice(&frame.pixels_argb);
                } else if buffer.len() == self.blank.len() {
                    buffer.copy_from_slice(&self.blank);
                } else {
                    for px in buffer.iter_mut() {
                        *px = 0xFF10_1010;
                    }
                }
            } else if buffer.len() == self.blank.len() {
                buffer.copy_from_slice(&self.blank);
            } else {
                for px in buffer.iter_mut() {
                    *px = 0xFF10_1010;
                }
            }

            buffer
                .present()
                .map_err(|err| format!("softbuffer present failed: {err}"))?;
            Ok(())
        })();

        self.surface = Some(surface);

        if let Err(err) = draw_result {
            self.fail(event_loop, err);
        }
    }
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title(DEFAULT_WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(
                DEFAULT_WINDOW_WIDTH as f64,
                DEFAULT_WINDOW_HEIGHT as f64,
            ));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(err) => {
                self.fail(event_loop, format!("failed to create window: {err}"));
                return;
            }
        };

        let context = match SoftbufferContext::new(Arc::clone(&window)) {
            Ok(c) => c,
            Err(err) => {
                self.fail(
                    event_loop,
                    format!("failed to create softbuffer context: {err}"),
                );
                return;
            }
        };

        let surface = match SoftbufferSurface::new(&context, Arc::clone(&window)) {
            Ok(s) => s,
            Err(err) => {
                self.fail(
                    event_loop,
                    format!("failed to create softbuffer surface: {err}"),
                );
                return;
            }
        };

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.last_seq = 0;
        self.last_frame = None;
        self.surface_size = (0, 0);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.stop_flag.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }

        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                self.stop_flag.store(true, Ordering::Relaxed);
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let is_escape = matches!(event.logical_key, Key::Named(NamedKey::Escape));
                    let is_q = matches!(&event.logical_key, Key::Character(ch) if ch.eq_ignore_ascii_case("q"));
                    if is_escape || is_q {
                        self.stop_flag.store(true, Ordering::Relaxed);
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.stop_flag.load(Ordering::Relaxed) {
            event_loop.exit();
            return;
        }
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

pub(crate) fn run_wayland_window(
    latest_frame: Arc<Mutex<Option<DecodedFrame>>>,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ViewerApp::new(latest_frame, stop_flag);
    event_loop
        .run_app(&mut app)
        .map_err(|err| anyhow!("winit event loop failed: {err}"))?;

    if let Some(err) = app.fatal_error.take() {
        bail!("{err}");
    }
    Ok(())
}

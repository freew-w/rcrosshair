use super::crosshair::*;
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    registry::RegistryState,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{
        Shm, ShmHandler,
        slot::{ActivateSlotError, CreateBufferError, SlotPool},
    },
};
use std::{num::NonZeroU32, time::Instant};
use thiserror::Error;
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_output, wl_shm, wl_surface},
};

pub struct App {
    pub registry_state: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,

    pub exit: bool,
    pub first_configure: bool,
    pub pool: SlotPool,
    pub width: u32,
    pub height: u32,
    pub layer: LayerSurface,

    pub image: CrosshairImage,
    pub target_x: u32,
    pub target_y: u32,
    pub positioned: bool,
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        match self.image {
            CrosshairImage::Gif(ref mut image) => {
                let now = Instant::now();
                let elapsed = now.duration_since(image.last_frame_time);
                let delay_ms = image.frames[image.current_frame].delay_ms;

                if elapsed.as_millis() >= delay_ms {
                    image.current_frame = (image.current_frame + 1) % image.frames.len();
                    image.last_frame_time = now;
                }

                if let Err(e) = self.draw(qh) {
                    log::error!("Failed to draw frame: {}", e);
                }
            }
            CrosshairImage::Static(_) => {
                // Ignore
            }
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let new_w = NonZeroU32::new(configure.new_size.0).map_or(self.width, NonZeroU32::get);
        let new_h = NonZeroU32::new(configure.new_size.1).map_or(self.height, NonZeroU32::get);

        // Resize pool only if significantly larger
        let needed = (new_w * new_h * 4) as usize;
        if needed > self.pool.len() {
            if let Ok(new_pool) = SlotPool::new(
                needed.max((self.width * self.height * 4) as usize),
                &self.shm,
            ) {
                self.pool = new_pool;
            } else {
                log::error!("Failed to resize shm pool");
            }
        }

        let size_changed = new_w != self.width || new_h != self.height;

        self.width = new_w;
        self.height = new_h;

        if let Some(output) = self.output_state.outputs().next()
            && let Some(info) = self.output_state.info(&output)
        {
            let (screen_w, screen_h) = info.logical_size.unwrap_or((1920, 1080));

            let left_margin = (screen_w / 2) - self.target_x as i32;
            let top_margin = (screen_h / 2) - self.target_y as i32;

            self.layer.set_anchor(Anchor::TOP | Anchor::LEFT);
            self.layer.set_margin(top_margin, 0, 0, left_margin);
            self.positioned = true;
            self.layer.commit();
        }

        if self.first_configure || size_changed {
            if !self.positioned {
                self.layer.commit();
            }

            self.first_configure = false;
            if let Err(e) = self.draw(qh) {
                log::error!("Draw failed after configure: {}", e);
            }
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

#[derive(Debug, Error)]
enum DrawError {
    #[error("Failed to create buffer: {0}")]
    CreateBuffer(#[from] CreateBufferError),
    #[error("Failed to activate slot: {0}")]
    ActivateSlot(#[from] ActivateSlotError),
}

impl App {
    fn draw(&mut self, qh: &QueueHandle<Self>) -> Result<(), DrawError> {
        let width = self.width;
        let height = self.height;
        let stride = self.width * 4;

        let (buffer, canvas) = self.pool.create_buffer(
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
        )?;

        // Draw to the window:
        // Clear canvas to transparent black
        canvas.fill(0);

        match self.image {
            CrosshairImage::Gif(ref crosshair) => {
                let frame = &crosshair.frames[crosshair.current_frame];
                canvas[..frame.data.len()].copy_from_slice(&frame.data);

                // Request our next frame
                self.layer
                    .wl_surface()
                    .frame(qh, self.layer.wl_surface().clone());
            }
            CrosshairImage::Static(ref frame) => {
                canvas[..frame.data.len()].copy_from_slice(&frame.data);
            }
        }

        // Damage the entire window
        self.layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);

        // Attach and commit to present.
        buffer.attach_to(self.layer.wl_surface())?;
        self.layer.commit();

        Ok(())
    }
}

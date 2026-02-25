use app::*;
use clap::Parser;
use crosshair::*;
use image::{
    AnimationDecoder, GenericImageView, ImageDecoder, ImageFormat, RgbaImage,
    codecs::gif::GifDecoder,
};
use smithay_client_toolkit::{
    compositor::{CompositorState, Region},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::OutputState,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{KeyboardInteractivity, Layer, LayerShell},
    },
    shm::{Shm, slot::SlotPool},
};
use std::{fs::File, io::BufReader, time::Instant};
use wayland_client::{Connection, globals::registry_queue_init};

mod app;
mod crosshair;

#[derive(Parser)]
struct Args {
    image_path: String,

    /// The x coordinate on the image to be centered
    #[arg(short = 'x', long)]
    target_x: Option<u32>,

    /// The y coordinate on the image to be centered
    #[arg(short = 'y', long)]
    target_y: Option<u32>,

    /// range from 0 to 1
    #[arg(short, long, default_value_t = 1f32)]
    opacity: f32,
}

fn process_buffer(buffer: RgbaImage, opacity: f32) -> Vec<u8> {
    let (w, h) = buffer.dimensions();
    let mut data = Vec::with_capacity((w * h * 4) as usize);

    for pixel in buffer.pixels() {
        let [r, g, b, a] = pixel.0;

        // Calculate the premultiplied alpha
        let alpha_f = (a as f32 * opacity) / 255f32;
        let new_a = (a as f32 * opacity) as u8;
        let new_r = (r as f32 * alpha_f) as u8;
        let new_g = (g as f32 * alpha_f) as u8;
        let new_b = (b as f32 * alpha_f) as u8;

        data.extend_from_slice(&[new_b, new_g, new_r, new_a]);
    }
    data
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Load the image
    let args = Args::parse();

    let image_path = &args.image_path;
    let image_reader = image::ImageReader::open(image_path)?;
    let format = image_reader.format().ok_or("Failed to read image format")?;

    let (image_w, image_h, image) = match format {
        ImageFormat::Gif => {
            let file_in = BufReader::new(File::open(image_path)?);
            let decoder = GifDecoder::new(file_in)?;

            let (w, h) = decoder.dimensions();
            let frames = decoder
                .into_frames()
                .collect_frames()?
                .into_iter()
                .map(|frame| {
                    let delay_ms = frame.delay().numer_denom_ms().0 as u128;
                    let buffer = frame.into_buffer();
                    let data = process_buffer(buffer, args.opacity);

                    GifFrame { data, delay_ms }
                })
                .collect();

            (
                w,
                h,
                CrosshairImage::Gif(GifImage {
                    frames,
                    current_frame: 0,
                    last_frame_time: Instant::now(),
                }),
            )
        }
        _ => {
            let image = image_reader.decode()?;
            let (w, h) = image.dimensions();
            let data = process_buffer(image.to_rgba8(), args.opacity);

            (w, h, CrosshairImage::Static(Frame { data }))
        }
    };

    // All Wayland apps start by connecting the compositor (server).
    let conn = Connection::connect_to_env()?;

    // Enumerate the list of globals to get the protocols the server implements.
    let (globals, mut event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    // The compositor (not to be confused with the server which is commonly called the compositor) allows
    // configuring surfaces to be presented.
    let compositor = CompositorState::bind(&globals, &qh)?;
    // This app uses the wlr layer shell, which may not be available with every compositor.
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    // Since we are not using the GPU in this example, we use wl_shm to allow software rendering to a buffer
    // we share with the compositor process.
    let shm = Shm::bind(&globals, &qh)?;

    // A layer surface is created from a surface.
    let surface = compositor.create_surface(&qh);
    // And then we create the layer shell.
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("rcrosshair"), None);
    // Configure the layer surface, providing things like the anchor on screen, desired size and the keyboard
    // interactivity
    let region = Region::new(&compositor)?;
    let wl_region = region.wl_region();
    layer.wl_surface().set_input_region(Some(wl_region));
    wl_region.destroy();

    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(image_w, image_h);

    // In order for the layer surface to be mapped, we need to perform an initial commit with no attached\
    // buffer. For more info, see WaylandSurface::commit
    //
    // The compositor will respond with an initial configure that we can then use to present to the layer
    // surface with the correct options.
    layer.commit();

    let pool = SlotPool::new((image_w * image_h * 4) as usize, &shm)?;
    let target_x = args.target_x.unwrap_or(image_w / 2);
    let target_y = args.target_y.unwrap_or(image_h / 2);
    let mut rcrosshair = App {
        // Seats and outputs may be hotplugged at runtime, therefore we need to setup a registry state to
        // listen for seats and outputs.
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,

        exit: false,
        first_configure: true,
        pool,
        width: image_w,
        height: image_h,
        layer,

        image,
        target_x,
        target_y,
        positioned: false,
    };

    // We don't draw immediately, the configure will notify us when to first draw.
    loop {
        event_queue.blocking_dispatch(&mut rcrosshair)?;

        if rcrosshair.exit {
            break;
        }
    }

    Ok(())
}

delegate_compositor!(App);
delegate_output!(App);
delegate_shm!(App);
delegate_layer!(App);
delegate_registry!(App);
impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

use app::*;
use cache_params::*;
use clap::Parser;
use crosshair::*;
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
use wayland_client::{Connection, globals::registry_queue_init};

mod app;
mod cache_params;
mod crosshair;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Load the image
    let args = Args::parse();

    let image_hash = compute_image_hash(&args.image_path)?;

    let cache_path = get_cache_path()
        .map(|p| p.join("cache"))
        .unwrap_or("cache".into());
    let mut cache = load_cache(&cache_path);
    let cache_entry = cache.history.get(&image_hash);

    if Some(Commands::Clear) == args.command {
        match cache.history.remove(&image_hash) {
            Some(v) => println!("Removed cached parameters: {:?}", v),
            None => println!("Cached parameters not found, nothing was removed"),
        }
        save_cache(&cache_path, &cache)?;

        return Ok(());
    };

    let opacity = args
        .opacity
        .or(cache_entry.map(|e| e.opacity))
        .unwrap_or(1f32);

    let (image_w, image_h, image) = load_image(&args.image_path, opacity)?;

    let target_x = args
        .target_x
        .or(cache_entry.map(|e| e.target_x))
        .unwrap_or(image_w / 2);

    let target_y = args
        .target_y
        .or(cache_entry.map(|e| e.target_y))
        .unwrap_or(image_h / 2);

    cache.history.insert(
        image_hash,
        CachedParams {
            path_for_readability: args.image_path,
            target_x,
            target_y,
            opacity,
        },
    );

    save_cache(&cache_path, &cache)?;

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

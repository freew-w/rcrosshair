use image::{ImageBuffer, Rgba};
use std::time::Instant;

pub struct GifFrame {
    pub data: Vec<u8>,
    pub delay_ms: u128,
}

pub struct GifImage {
    pub frames: Vec<GifFrame>,
    pub current_frame: usize,
    pub last_frame_time: Instant,
}

pub enum CrosshairImage {
    Static(ImageBuffer<Rgba<u8>, Vec<u8>>),
    Gif(GifImage),
}

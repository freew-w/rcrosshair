use image::{
    AnimationDecoder, GenericImageView, ImageDecoder, ImageFormat, RgbaImage,
    codecs::gif::GifDecoder,
};
use std::{
    fs::File,
    io::{self, BufReader},
    path::Path,
    time::Instant,
};

pub struct GifFrame {
    pub data: Vec<u8>,
    pub delay_ms: u128,
}

pub struct Frame {
    pub data: Vec<u8>,
}

pub struct GifImage {
    pub frames: Vec<GifFrame>,
    pub current_frame: usize,
    pub last_frame_time: Instant,
}

pub enum CrosshairImage {
    Static(Frame),
    Gif(GifImage),
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

#[derive(thiserror::Error, Debug)]
pub enum LoadImageError {
    #[error("Failed to load image: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to load image: {0}")]
    Image(#[from] image::error::ImageError),

    #[error("Failed to detect image format")]
    UnknownFormat,
}

pub fn load_image(
    path: impl AsRef<Path>,
    opacity: f32,
) -> Result<(u32, u32, CrosshairImage), LoadImageError> {
    let path = path.as_ref();
    let reader = image::ImageReader::open(path)?;
    let format = reader.format().ok_or(LoadImageError::UnknownFormat)?;

    let (w, h, image) = match format {
        ImageFormat::Gif => {
            let file_in = BufReader::new(File::open(path)?);
            let decoder = GifDecoder::new(file_in)?;

            let (w, h) = decoder.dimensions();
            let frames = decoder
                .into_frames()
                .collect_frames()?
                .into_iter()
                .map(|frame| {
                    let delay_ms = frame.delay().numer_denom_ms().0 as u128;
                    let buffer = frame.into_buffer();
                    let data = process_buffer(buffer, opacity);

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
            let image = reader.decode()?;
            let (w, h) = image.dimensions();
            let data = process_buffer(image.to_rgba8(), opacity);

            (w, h, CrosshairImage::Static(Frame { data }))
        }
    };

    Ok((w, h, image))
}

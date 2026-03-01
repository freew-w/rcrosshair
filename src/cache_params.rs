use blake3::Hasher;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

#[derive(Parser)]
pub struct Args {
    pub image_path: String,

    /// The x coordinate on the image to be centered
    #[arg(short = 'x', long)]
    pub target_x: Option<u32>,

    /// The y coordinate on the image to be centered
    #[arg(short = 'y', long)]
    pub target_y: Option<u32>,

    /// range from 0 to 1
    #[arg(short, long)]
    pub opacity: Option<f32>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, PartialEq)]
pub enum Commands {
    /// Clear cached parameters for an image
    Clear,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Cache {
    pub history: HashMap<String, CachedParams>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CachedParams {
    pub path_for_readability: String,
    pub target_x: u32,
    pub target_y: u32,
    pub opacity: f32,
}

pub fn get_cache_path() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("rcrosshair"))
}

pub fn load_cache(path: impl AsRef<Path>) -> Cache {
    let data = fs::read_to_string(path).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save_cache(path: impl AsRef<Path>, cache: &Cache) -> io::Result<()> {
    if !path.as_ref().parent().unwrap().exists() {
        fs::create_dir(path.as_ref().parent().unwrap())?;
    }

    if let Ok(json) = serde_json::to_string_pretty(cache) {
        fs::write(path, json)?;
    }

    Ok(())
}

pub fn compute_image_hash(path: impl AsRef<Path>) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Hasher::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_string())
}

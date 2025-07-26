use anyhow::{anyhow, Result};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::collections::HashMap;
use std::path::PathBuf;
use ratatui::style::Color;

/// Represents ANSI color codes for terminal display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnsiColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl AnsiColor {
    /// Convert to ratatui Color
    pub fn to_ratatui_color(&self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }
}

/// A pixelated representation of album art using ANSI colors
#[derive(Debug, Clone)]
pub struct PixelatedAlbumArt {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Vec<AnsiColor>>,
}

impl PixelatedAlbumArt {
    /// Create a new pixelated album art from raw pixel data
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![vec![AnsiColor { r: 0, g: 0, b: 0 }; width as usize]; height as usize],
        }
    }
}

/// Manager for album art caching and processing
pub struct AlbumArtManager {
    cache_dir: PathBuf,
    // Cache of album URL to processed art
    cache: HashMap<String, PixelatedAlbumArt>,
}

impl AlbumArtManager {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow!("Could not find cache directory"))?
            .join("spotify-tui")
            .join("album-art");
        
        std::fs::create_dir_all(&cache_dir)?;
        
        Ok(Self {
            cache_dir,
            cache: HashMap::new(),
        })
    }

    /// Download and process album art from URL
    pub async fn get_album_art(&mut self, url: &str, target_size: u32) -> Result<PixelatedAlbumArt> {
        // Check cache first
        if let Some(cached) = self.cache.get(url) {
            return Ok(cached.clone());
        }

        // Download image
        let image_data = self.download_image(url).await?;
        
        // Process into pixelated art
        let pixelated = self.pixelate_image(image_data, target_size)?;
        
        // Cache the result
        self.cache.insert(url.to_string(), pixelated.clone());
        
        Ok(pixelated)
    }

    /// Download image from URL
    async fn download_image(&self, url: &str) -> Result<DynamicImage> {
        // Create a new reqwest client
        let client = reqwest::Client::new();
        let response = client.get(url).send().await?;
        let bytes = response.bytes().await?;
        let image = image::load_from_memory(&bytes)?;
        Ok(image)
    }

    /// Convert image to pixelated ANSI art
    fn pixelate_image(&self, image: DynamicImage, target_size: u32) -> Result<PixelatedAlbumArt> {
        // Resize image to target size (maintaining aspect ratio)
        let resized = image.resize_exact(target_size, target_size, image::imageops::FilterType::Nearest);
        
        let mut art = PixelatedAlbumArt::new(target_size, target_size);
        
        // Convert each pixel to ANSI color
        for y in 0..target_size {
            for x in 0..target_size {
                let pixel = resized.get_pixel(x, y);
                let Rgba([r, g, b, _]) = pixel;
                
                // Convert to ANSI color (we could do color quantization here for better terminal support)
                art.pixels[y as usize][x as usize] = AnsiColor { r, g, b };
            }
        }
        
        Ok(art)
    }

    /// Get a placeholder art for when no album art is available
    pub fn get_placeholder_art(size: u32) -> PixelatedAlbumArt {
        let mut art = PixelatedAlbumArt::new(size, size);
        
        // Create a simple pattern
        for y in 0..size {
            for x in 0..size {
                let color = if (x + y) % 2 == 0 {
                    AnsiColor { r: 40, g: 40, b: 40 }
                } else {
                    AnsiColor { r: 60, g: 60, b: 60 }
                };
                art.pixels[y as usize][x as usize] = color;
            }
        }
        
        art
    }
}

/// Helper to render pixelated art as colored blocks
pub fn render_pixelated_art(art: &PixelatedAlbumArt) -> Vec<Vec<(String, Color)>> {
    let mut lines = Vec::new();
    
    for row in &art.pixels {
        let mut line = Vec::new();
        for pixel in row {
            // Use single block character for normal view
            line.push(("█".to_string(), pixel.to_ratatui_color()));
        }
        lines.push(line);
    }
    
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_color_conversion() {
        let color = AnsiColor { r: 255, g: 128, b: 0 };
        let ratatui_color = color.to_ratatui_color();
        assert_eq!(ratatui_color, Color::Rgb(255, 128, 0));
    }

    #[test]
    fn test_placeholder_art() {
        let art = AlbumArtManager::get_placeholder_art(8);
        assert_eq!(art.width, 8);
        assert_eq!(art.height, 8);
        assert_eq!(art.pixels.len(), 8);
        assert_eq!(art.pixels[0].len(), 8);
    }
}
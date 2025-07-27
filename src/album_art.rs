use anyhow::{anyhow, Result};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use std::collections::HashMap;
use std::path::PathBuf;
use ratatui::style::Color;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

/// Represents ANSI color codes for terminal display
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Cached art entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedArt {
    art: PixelatedAlbumArt,
    timestamp: u64,
    size: u32,
}

/// Manager for album art caching and processing
pub struct AlbumArtManager {
    cache_dir: PathBuf,
    // In-memory cache of album URL to processed art
    memory_cache: HashMap<String, CachedArt>,
    // Maximum number of items in memory cache
    max_memory_items: usize,
    // Maximum age for disk cache in seconds (7 days)
    max_cache_age: u64,
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
            memory_cache: HashMap::new(),
            max_memory_items: 50,
            max_cache_age: 7 * 24 * 60 * 60, // 7 days
        })
    }

    /// Download and process album art from URL
    pub async fn get_album_art(&mut self, url: &str, target_size: u32) -> Result<PixelatedAlbumArt> {
        let cache_key = format!("{}-{}", url, target_size);
        
        // Check memory cache first
        if let Some(cached) = self.memory_cache.get(&cache_key) {
            if cached.size == target_size {
                return Ok(cached.art.clone());
            }
        }
        
        // Check disk cache
        if let Ok(cached) = self.load_from_disk_cache(&cache_key) {
            if cached.size == target_size && self.is_cache_valid(cached.timestamp) {
                // Add to memory cache
                self.add_to_memory_cache(cache_key.clone(), cached.clone());
                return Ok(cached.art);
            }
        }

        // Download image
        let image_data = self.download_image(url).await?;
        
        // Process into pixelated art
        let pixelated = self.pixelate_image(image_data, target_size)?;
        
        // Create cached entry
        let cached = CachedArt {
            art: pixelated.clone(),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            size: target_size,
        };
        
        // Save to disk cache
        let _ = self.save_to_disk_cache(&cache_key, &cached);
        
        // Add to memory cache
        self.add_to_memory_cache(cache_key, cached);
        
        Ok(pixelated)
    }

    /// Download image from URL with timeout
    async fn download_image(&self, url: &str) -> Result<DynamicImage> {
        // Create a new reqwest client with timeout
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        
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

    /// Add to memory cache with LRU eviction
    fn add_to_memory_cache(&mut self, key: String, cached: CachedArt) {
        // Evict oldest entries if cache is full
        if self.memory_cache.len() >= self.max_memory_items {
            // Find oldest entry
            if let Some(oldest_key) = self.memory_cache
                .iter()
                .min_by_key(|(_, v)| v.timestamp)
                .map(|(k, _)| k.clone())
            {
                self.memory_cache.remove(&oldest_key);
            }
        }
        
        self.memory_cache.insert(key, cached);
    }
    
    /// Check if cache timestamp is still valid
    fn is_cache_valid(&self, timestamp: u64) -> bool {
        if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
            now.as_secs() - timestamp < self.max_cache_age
        } else {
            false
        }
    }
    
    /// Get cache file path for a key
    fn get_cache_path(&self, key: &str) -> PathBuf {
        // Create a safe filename from the key
        let safe_key = key.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
            .collect::<String>();
        self.cache_dir.join(format!("{}.json", safe_key))
    }
    
    /// Load from disk cache
    fn load_from_disk_cache(&self, key: &str) -> Result<CachedArt> {
        let path = self.get_cache_path(key);
        let data = std::fs::read_to_string(path)?;
        let cached: CachedArt = serde_json::from_str(&data)?;
        Ok(cached)
    }
    
    /// Save to disk cache
    fn save_to_disk_cache(&self, key: &str, cached: &CachedArt) -> Result<()> {
        let path = self.get_cache_path(key);
        let data = serde_json::to_string(cached)?;
        std::fs::write(path, data)?;
        Ok(())
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
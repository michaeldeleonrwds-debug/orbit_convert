use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchedFolder {
    pub id: String,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub is_watched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversionPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub target_format: String,
    pub quality: u8,
    pub svg_scale: f32,
    pub bg_color: [u8; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Keyboard shortcut mode. Default is "Alt" (holding Alt).
    pub shortcut: String,
    /// Radial wheel outer radius in logical pixels.
    pub wheel_radius: f32,
    /// Inner neutral dead zone radius in logical pixels.
    pub dead_zone_radius: f32,
    /// UI primary accent color (hex string, e.g. "#3b82f6").
    pub accent_color: String,
    /// Default JPEG quality (1-100).
    pub jpeg_quality: u8,
    /// Default WebP quality (0-100).
    pub webp_quality: f32,
    /// Default SVG scaling factor (e.g. 1.0 or 2.0 for retina).
    pub svg_scale: f32,
    /// RGB background color for compositing transparency in JPEG (default white: [255, 255, 255]).
    pub jpeg_bg_color: [u8; 3],
    /// Custom destination directory if not saving alongside the source file.
    pub custom_output_dir: Option<PathBuf>,
    /// Show Windows desktop notifications on conversion completion/error.
    pub show_notifications: bool,
    /// Launch OrbitConvert automatically on Windows startup.
    pub launch_on_startup: bool,

    /// Watched & favorite folders list
    #[serde(default = "default_folders")]
    pub folders: Vec<WatchedFolder>,

    /// Reusable conversion presets
    #[serde(default = "default_presets")]
    pub presets: Vec<ConversionPreset>,

    /// Output destination mode: "source", "custom", "source_converted"
    #[serde(default = "default_output_mode")]
    pub output_mode: String,

    /// Open output folder after conversion completes
    #[serde(default)]
    pub open_after_conversion: bool,

    /// Minimize main window to system tray instead of exiting
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
}

fn default_true() -> bool {
    true
}

fn default_output_mode() -> String {
    "source".to_string()
}

fn default_folders() -> Vec<WatchedFolder> {
    let mut list = Vec::new();
    if let Some(user_dirs) = directories::UserDirs::new() {
        if let Some(downloads) = user_dirs.download_dir() {
            list.push(WatchedFolder {
                id: "folder_downloads".into(),
                name: "Downloads".into(),
                path: downloads.to_string_lossy().into_owned(),
                enabled: true,
                is_watched: true,
            });
        }
        if let Some(pictures) = user_dirs.picture_dir() {
            list.push(WatchedFolder {
                id: "folder_pictures".into(),
                name: "Pictures".into(),
                path: pictures.to_string_lossy().into_owned(),
                enabled: true,
                is_watched: false,
            });
        }
        let docs = user_dirs.document_dir();
        list.push(WatchedFolder {
            id: "folder_documents".into(),
            name: "Documents".into(),
            path: docs.map(|d| d.to_string_lossy().into_owned()).unwrap_or_else(|| "C:\\Users".into()),
            enabled: false,
            is_watched: false,
        });
    } else {
        list.push(WatchedFolder {
            id: "folder_downloads".into(),
            name: "Downloads".into(),
            path: "C:\\Users\\Default\\Downloads".into(),
            enabled: true,
            is_watched: true,
        });
    }
    list
}

fn default_presets() -> Vec<ConversionPreset> {
    vec![
        ConversionPreset {
            id: "preset_web_optimized".into(),
            name: "Web Optimized".into(),
            description: "High-compression WebP format with 85% quality for fast web delivery".into(),
            target_format: "WEBP".into(),
            quality: 85,
            svg_scale: 1.0,
            bg_color: [255, 255, 255],
        },
        ConversionPreset {
            id: "preset_hq_jpeg".into(),
            name: "High Quality JPEG".into(),
            description: "95% quality JPEG composited onto clean white background".into(),
            target_format: "JPG".into(),
            quality: 95,
            svg_scale: 1.0,
            bg_color: [255, 255, 255],
        },
        ConversionPreset {
            id: "preset_lossless_png".into(),
            name: "Lossless Transparent PNG".into(),
            description: "Full alpha channel transparency preserved with lossless PNG compression".into(),
            target_format: "PNG".into(),
            quality: 100,
            svg_scale: 1.0,
            bg_color: [255, 255, 255],
        },
        ConversionPreset {
            id: "preset_windows_icon".into(),
            name: "Windows Multi-Resolution Icon".into(),
            description: "Standard Windows ICO format with 16, 32, 48, 64, 128, 256 mip-levels".into(),
            target_format: "ICO".into(),
            quality: 100,
            svg_scale: 1.0,
            bg_color: [255, 255, 255],
        },
        ConversionPreset {
            id: "preset_retina_vector".into(),
            name: "Retina Vector Raster (2x)".into(),
            description: "SVG rendered at 2.0x scale to crystal-clear high-DPI PNG".into(),
            target_format: "PNG".into(),
            quality: 100,
            svg_scale: 2.0,
            bg_color: [255, 255, 255],
        },
    ]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            shortcut: "Alt".to_string(),
            wheel_radius: 170.0,
            dead_zone_radius: 46.0,
            accent_color: "#3b82f6".to_string(),
            jpeg_quality: 90,
            webp_quality: 85.0,
            svg_scale: 1.0,
            jpeg_bg_color: [255, 255, 255],
            custom_output_dir: None,
            show_notifications: true,
            launch_on_startup: false,
            folders: default_folders(),
            presets: default_presets(),
            output_mode: default_output_mode(),
            open_after_conversion: false,
            minimize_to_tray: true,
        }
    }
}

impl AppConfig {
    pub fn config_file_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "orbitconvert", "OrbitConvert")
            .map(|proj| proj.config_dir().join("settings.toml"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_file_path() {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(content) => match toml::from_str::<AppConfig>(&content) {
                        Ok(config) => {
                            info!("Loaded settings from {}", path.display());
                            return config;
                        }
                        Err(e) => {
                            warn!("Failed to parse config at {}: {}. Using defaults.", path.display(), e);
                        }
                    },
                    Err(e) => {
                        warn!("Failed to read config at {}: {}. Using defaults.", path.display(), e);
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        if let Some(path) = Self::config_file_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let toml_str = toml::to_string_pretty(self)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            fs::write(&path, toml_str)?;
            info!("Saved settings to {}", path.display());
        }
        Ok(())
    }
}

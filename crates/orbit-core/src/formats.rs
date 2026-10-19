use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Svg,
    Png,
    Jpg,
    Webp,
    Ico,
    Bmp,
    Tiff,
}

impl FileFormat {
    /// Attempt to identify the file format by its file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean.as_str() {
            "svg" => Some(FileFormat::Svg),
            "png" => Some(FileFormat::Png),
            "jpg" | "jpeg" | "jpe" | "jfif" => Some(FileFormat::Jpg),
            "webp" => Some(FileFormat::Webp),
            "ico" => Some(FileFormat::Ico),
            "bmp" => Some(FileFormat::Bmp),
            "tif" | "tiff" => Some(FileFormat::Tiff),
            _ => None,
        }
    }

    /// Identify the format from a file path.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        path.as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Primary file extension for this format without leading dot.
    pub fn primary_extension(&self) -> &'static str {
        match self {
            FileFormat::Svg => "svg",
            FileFormat::Png => "png",
            FileFormat::Jpg => "jpg",
            FileFormat::Webp => "webp",
            FileFormat::Ico => "ico",
            FileFormat::Bmp => "bmp",
            FileFormat::Tiff => "tiff",
        }
    }

    /// Human-readable label (e.g. for radial wheel button).
    pub fn display_label(&self) -> &'static str {
        match self {
            FileFormat::Svg => "SVG",
            FileFormat::Png => "PNG",
            FileFormat::Jpg => "JPG",
            FileFormat::Webp => "WEBP",
            FileFormat::Ico => "ICO",
            FileFormat::Bmp => "BMP",
            FileFormat::Tiff => "TIFF",
        }
    }

    /// MIME type for the format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            FileFormat::Svg => "image/svg+xml",
            FileFormat::Png => "image/png",
            FileFormat::Jpg => "image/jpeg",
            FileFormat::Webp => "image/webp",
            FileFormat::Ico => "image/x-icon",
            FileFormat::Bmp => "image/bmp",
            FileFormat::Tiff => "image/tiff",
        }
    }

    /// Whether this format inherently supports alpha transparency.
    pub fn supports_transparency(&self) -> bool {
        match self {
            FileFormat::Svg | FileFormat::Png | FileFormat::Webp | FileFormat::Ico => true,
            FileFormat::Jpg | FileFormat::Bmp | FileFormat::Tiff => false,
        }
    }
}

impl fmt::Display for FileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(FileFormat::from_extension("png"), Some(FileFormat::Png));
        assert_eq!(FileFormat::from_extension(".PNG"), Some(FileFormat::Png));
        assert_eq!(FileFormat::from_extension("jpeg"), Some(FileFormat::Jpg));
        assert_eq!(FileFormat::from_extension("jpg"), Some(FileFormat::Jpg));
        assert_eq!(FileFormat::from_extension("svg"), Some(FileFormat::Svg));
        assert_eq!(FileFormat::from_extension("webp"), Some(FileFormat::Webp));
        assert_eq!(FileFormat::from_extension("ico"), Some(FileFormat::Ico));
        assert_eq!(FileFormat::from_extension("unknown"), None);
    }

    #[test]
    fn test_format_from_path() {
        assert_eq!(FileFormat::from_path("C:\\test\\image.svg"), Some(FileFormat::Svg));
        assert_eq!(FileFormat::from_path("/path/to/icon.ICO"), Some(FileFormat::Ico));
        assert_eq!(FileFormat::from_path("no_ext"), None);
    }
}

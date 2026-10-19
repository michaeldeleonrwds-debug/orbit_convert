use crate::formats::{FileFormat, FormatCategory};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOptions {
    /// JPEG quality factor (1-100). Default is 90.
    pub jpeg_quality: u8,
    /// WebP quality factor (0.0 - 100.0). Default is 85.0.
    pub webp_quality: f32,
    /// WebP lossless encoding flag. Default is false.
    pub webp_lossless: bool,
    /// SVG scaling factor. 1.0 means original viewBox/dimensions.
    pub svg_scale: f32,
    /// Explicit SVG target width if specified.
    pub svg_target_width: Option<u32>,
    /// Explicit SVG target height if specified.
    pub svg_target_height: Option<u32>,
    /// RGB color used to composite transparent alpha channels when converting to non-alpha formats like JPEG.
    /// Default is pure white: (255, 255, 255).
    pub background_color: (u8, u8, u8),
    /// Target icon sizes when generating ICO. Default is [16, 32, 48, 64, 128, 256].
    pub ico_sizes: Vec<u32>,

    // Video & Audio Transcoding Options
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub video_crf: Option<u32>,
    pub video_bitrate: Option<String>,
    pub audio_bitrate: Option<String>,
    pub resolution: Option<String>,
    pub frame_rate: Option<f32>,
    pub hardware_accel: Option<String>,
    pub stream_copy: bool,
    pub preserve_metadata: bool,
    pub preserve_subtitles: bool,
    #[serde(skip)]
    pub cancel_flag: Option<Arc<AtomicBool>>,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            jpeg_quality: 90,
            webp_quality: 85.0,
            webp_lossless: false,
            svg_scale: 1.0,
            svg_target_width: None,
            svg_target_height: None,
            background_color: (255, 255, 255),
            ico_sizes: vec![16, 32, 48, 64, 128, 256],
            video_codec: None,
            audio_codec: None,
            video_crf: None,
            video_bitrate: None,
            audio_bitrate: None,
            resolution: None,
            frame_rate: None,
            hardware_accel: None,
            stream_copy: false,
            preserve_metadata: true,
            preserve_subtitles: true,
            cancel_flag: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Invalid or corrupted input file: {0}")]
    InvalidInput(String),

    #[error("Unsupported conversion from {from} to {to}")]
    UnsupportedConversion {
        from: FileFormat,
        to: FileFormat,
    },

    #[error("Conversion processing failed: {0}")]
    ProcessingError(String),

    #[error("Filesystem I/O error: {0}")]
    IoError(String),

    #[error("Conversion was cancelled")]
    Cancelled,
}

pub type ProgressReporter<'a> = Option<&'a dyn Fn(f32)>;

/// Trait implemented by conversion adapters (image-rs, resvg, ffmpeg, external tools, etc.).
pub trait Converter: Send + Sync {
    /// Unique identifier for this converter adapter.
    fn id(&self) -> &'static str;

    /// Returns true if this converter can transform `from` into `to`.
    fn supports(&self, from: FileFormat, to: FileFormat) -> bool;

    /// Execute the file conversion.
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyStatus {
    Builtin,
    Active,
    Missing,
}

impl std::fmt::Display for DependencyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencyStatus::Builtin => write!(f, "Built-in (Rust Native)"),
            DependencyStatus::Active => write!(f, "Active (Engine Installed)"),
            DependencyStatus::Missing => write!(f, "Requires External Engine"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub format: FileFormat,
    pub can_decode: bool,
    pub can_encode: bool,
    pub engine: &'static str,
    pub status: DependencyStatus,
    pub dependency_help: &'static str,
    pub tested_status: &'static str,
    pub known_limitations: &'static str,
}

/// Registry mapping source formats to target formats and their responsible converter.
#[derive(Default, Clone)]
pub struct ConversionRegistry {
    converters: Vec<Arc<dyn Converter>>,
    routes: HashMap<FileFormat, HashSet<FileFormat>>,
}

impl ConversionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a converter implementation and index all supported routes.
    pub fn register(&mut self, converter: Arc<dyn Converter>) {
        let all_formats = Self::all_registered_formats();

        for &from in &all_formats {
            for &to in &all_formats {
                // A format should only be shown as destination if the conversion is valid
                if from != to && converter.supports(from, to) {
                    self.routes.entry(from).or_default().insert(to);
                }
            }
        }

        self.converters.push(converter);
    }

    /// Returns list of all known formats across the system.
    pub fn all_registered_formats() -> Vec<FileFormat> {
        vec![
            // Raster
            FileFormat::Png,
            FileFormat::Jpg,
            FileFormat::Webp,
            FileFormat::Ico,
            FileFormat::Bmp,
            FileFormat::Tiff,
            FileFormat::Gif,
            FileFormat::Tga,
            FileFormat::Dds,
            FileFormat::Ppm,
            FileFormat::Pcx,
            FileFormat::Dpx,
            // Modern Web
            FileFormat::Avif,
            FileFormat::Heic,
            FileFormat::Jxl,
            // Vector
            FileFormat::Svg,
            FileFormat::Eps,
            FileFormat::Art,
            FileFormat::Dcs,
            FileFormat::Wmz,
            FileFormat::Emz,
            // Camera RAW
            FileFormat::Cr2,
            FileFormat::Cr3,
            FileFormat::Crw,
            FileFormat::Nef,
            FileFormat::Nrw,
            FileFormat::Arw,
            FileFormat::Sr2,
            FileFormat::Srf,
            FileFormat::Raf,
            FileFormat::Orf,
            FileFormat::Rw2,
            FileFormat::Dng,
            FileFormat::Pef,
            FileFormat::Ptx,
            FileFormat::Rwl,
            FileFormat::Srw,
            FileFormat::X3f,
            FileFormat::Dcr,
            FileFormat::Drf,
            FileFormat::Kdc,
            FileFormat::K25,
            // Specialized & Archives
            FileFormat::Psd,
            FileFormat::Psb,
            FileFormat::Djvu,
            FileFormat::Cbr,
            FileFormat::Cbz,
            FileFormat::Odd,
            // Video
            FileFormat::Mp4,
            FileFormat::Mov,
            FileFormat::Mkv,
            FileFormat::Avi,
            FileFormat::Webm,
            FileFormat::Wmv,
            FileFormat::Asf,
            FileFormat::Flv,
            FileFormat::Mpeg,
            FileFormat::M4v,
            FileFormat::Ts,
            FileFormat::Mts,
            FileFormat::Vob,
            FileFormat::Mxf,
            FileFormat::ThreeGp,
            FileFormat::ThreeG2,
            FileFormat::Ogv,
            // Audio
            FileFormat::Mp3,
            FileFormat::Wav,
            FileFormat::Flac,
            FileFormat::Aac,
            FileFormat::M4a,
            FileFormat::Ogg,
            FileFormat::Opus,
            // Spreadsheets & Data
            FileFormat::Csv,
            FileFormat::Xlsx,
            FileFormat::Json,
            FileFormat::Tsv,
            // Documents
            FileFormat::Pdf,
            FileFormat::Docx,
            FileFormat::Txt,
            FileFormat::Html,
        ]
    }

    /// Retrieve all available target formats for a given source format and optional file path,
    /// inspecting streams via ffprobe to exclude audio extraction if no audio stream exists.
    pub fn get_available_targets_for_file(
        &self,
        source: FileFormat,
        path: Option<&Path>,
    ) -> Vec<FileFormat> {
        let mut targets: Vec<FileFormat> = self
            .routes
            .get(&source)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();

        // If source is a video file, check if it contains audio
        if source.is_video() {
            if let Some(p) = path {
                if let Some(info) = crate::probe::probe_media(p) {
                    if !info.has_audio {
                        // Source video has no audio track, so audio extraction is invalid
                        targets.retain(|fmt| !fmt.is_audio());
                    }
                }
            }
        }

        // Prioritize destinations depending on source type
        if source.is_video() {
            targets.sort_by_key(|fmt| match fmt {
                FileFormat::Mp4 => 1,
                FileFormat::Webm => 2,
                FileFormat::Mkv => 3,
                FileFormat::Mp3 => 4, // Audio extraction prioritized
                FileFormat::Gif => 5, // Animated GIF
                FileFormat::Mov => 6,
                FileFormat::Avi => 7,
                FileFormat::Webp => 8,
                FileFormat::Wmv => 9,
                FileFormat::Ts => 10,
                FileFormat::M4v => 11,
                FileFormat::Mpeg => 12,
                FileFormat::Flv => 13,
                FileFormat::Wav => 20,
                FileFormat::Flac => 21,
                FileFormat::Aac => 22,
                FileFormat::M4a => 23,
                FileFormat::Ogg => 24,
                FileFormat::Opus => 25,
                _ => 100,
            });
        } else if source.is_audio() {
            targets.sort_by_key(|fmt| match fmt {
                FileFormat::Mp3 => 1,
                FileFormat::Wav => 2,
                FileFormat::Flac => 3,
                FileFormat::Aac => 4,
                FileFormat::Ogg => 5,
                FileFormat::M4a => 6,
                FileFormat::Opus => 7,
                _ => 100,
            });
        } else {
            targets.sort_by_key(|fmt| match fmt {
                FileFormat::Png => 1,
                FileFormat::Jpg => 2,
                FileFormat::Webp => 3,
                FileFormat::Ico => 4,
                FileFormat::Bmp => 5,
                FileFormat::Tiff => 6,
                FileFormat::Gif => 7,
                FileFormat::Tga => 8,
                FileFormat::Ppm => 9,
                FileFormat::Avif => 10,
                FileFormat::Heic => 11,
                FileFormat::Jxl => 12,
                FileFormat::Svg => 13,
                FileFormat::Pdf => 14,
                _ => 100,
            });
        }

        targets
    }

    /// Retrieve all available target formats for a given source format, sorted by relevance.
    pub fn get_available_targets(&self, source: FileFormat) -> Vec<FileFormat> {
        self.get_available_targets_for_file(source, None)
    }

    /// Splits available targets into:
    /// - Default inner wheel (up to 5 formats + More if > 6; or up to 6 formats if <= 6)
    /// - Expanded outer wheel (the remaining formats)
    pub fn get_default_and_expanded_targets_for_file(
        &self,
        source: FileFormat,
        path: Option<&Path>,
    ) -> (Vec<FileFormat>, Vec<FileFormat>) {
        let all_targets = self.get_available_targets_for_file(source, path);
        if all_targets.len() <= 6 {
            (all_targets, Vec::new())
        } else {
            let inner = all_targets[..5].to_vec();
            let outer = all_targets[5..].to_vec();
            (inner, outer)
        }
    }

    /// Splits available targets into default inner wheel and expanded outer wheel without file context.
    pub fn get_default_and_expanded_targets(
        &self,
        source: FileFormat,
    ) -> (Vec<FileFormat>, Vec<FileFormat>) {
        self.get_default_and_expanded_targets_for_file(source, None)
    }

    /// Find an active converter that supports transforming `source` to `target`.
    pub fn find_converter(
        &self,
        source: FileFormat,
        target: FileFormat,
    ) -> Option<Arc<dyn Converter>> {
        self.converters
            .iter()
            .find(|c| c.supports(source, target))
            .cloned()
    }

    /// Returns a capability matrix covering every format in OrbitConvert.
    pub fn get_capability_matrix(&self) -> Vec<CapabilityInfo> {
        let all_formats = Self::all_registered_formats();
        let mut matrix = Vec::with_capacity(all_formats.len());

        for fmt in all_formats {
            // Check if any converter can decode this format
            let can_decode = self.converters.iter().any(|c| {
                Self::all_registered_formats()
                    .iter()
                    .any(|&t| c.supports(fmt, t))
            });

            // Check if any converter can encode this format
            let can_encode = self.converters.iter().any(|c| {
                Self::all_registered_formats()
                    .iter()
                    .any(|&s| c.supports(s, fmt))
            });

            let (engine, status, dep_help, tested, limitations) = match fmt.category() {
                FormatCategory::Raster => (
                    "Rust image-rs (Native)",
                    DependencyStatus::Builtin,
                    "Built into binary. Zero external dependencies.",
                    "Verified in Test Suite",
                    "Lossy compression for JPEG; Alpha transparency for PNG/WebP/ICO/TIFF.",
                ),
                FormatCategory::Vector => match fmt {
                    FileFormat::Svg => (
                        "resvg / usvg (Native)",
                        DependencyStatus::Builtin,
                        "Built into binary. Zero external dependencies.",
                        "Verified in Test Suite",
                        "Vector to raster rendering at configurable DPI/scale.",
                    ),
                    _ => (
                        "ImageMagick / FFmpeg Engine",
                        DependencyStatus::Active,
                        "Handled by external graphics engine.",
                        "Verified via CLI engine",
                        "Decodes PostScript/Vector paths to raster output.",
                    ),
                },
                FormatCategory::ModernWeb => (
                    "ImageMagick / FFmpeg Engine",
                    DependencyStatus::Active,
                    "Handled via FFmpeg/ImageMagick codec pipeline.",
                    "Runtime Verified",
                    "Next-gen container formats; quality depends on engine version.",
                ),
                FormatCategory::CameraRaw => (
                    "LibRaw / ImageMagick Engine",
                    DependencyStatus::Active,
                    "Raw camera sensor data decoded to full dynamic range RGB.",
                    "Verified Decode-Only",
                    "Decode only; camera RAW cannot be written as an export target.",
                ),
                FormatCategory::Specialized => match fmt {
                    FileFormat::Cbz | FileFormat::Cbr => (
                        "Archive & Image Extractor",
                        DependencyStatus::Builtin,
                        "Built-in archive unpacker and image serializer.",
                        "Verified Decode-Only",
                        "Extracts pages/covers to clean PNG/JPG/WebP.",
                    ),
                    _ => (
                        "Photoshop / Document Engine",
                        DependencyStatus::Active,
                        "Layer compositing via external adapter.",
                        "Verified Decode-Only",
                        "Flattened composite layer rendered.",
                    ),
                },
                FormatCategory::Video | FormatCategory::Audio => (
                    "FFmpeg Engine",
                    DependencyStatus::Active,
                    "FFmpeg must be installed on Windows system PATH.",
                    "Verified via Subprocess Pipe",
                    "Direct stream copy or high-efficiency re-encoding.",
                ),
                FormatCategory::Spreadsheet | FormatCategory::Data => (
                    "Rust Data Engine (Native)",
                    DependencyStatus::Builtin,
                    "Built-in CSV/TSV/JSON serializer.",
                    "Verified in Test Suite",
                    "Strict tabular structure preserved.",
                ),
                FormatCategory::Document => (
                    "Document Adapter",
                    DependencyStatus::Active,
                    "PDF rasterizer and plain text extraction.",
                    "Runtime Verified",
                    "Rasterizes first page for visual preview/export.",
                ),
            };

            matrix.push(CapabilityInfo {
                format: fmt,
                can_decode,
                can_encode,
                engine,
                status,
                dependency_help: dep_help,
                tested_status: tested,
                known_limitations: limitations,
            });
        }

        matrix
    }
}

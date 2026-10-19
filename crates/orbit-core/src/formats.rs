use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatCategory {
    Image,
    Video,
    Audio,
    Document,
    Spreadsheet,
    Data,
}

impl fmt::Display for FormatCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatCategory::Image => write!(f, "Images"),
            FormatCategory::Video => write!(f, "Video"),
            FormatCategory::Audio => write!(f, "Audio"),
            FormatCategory::Document => write!(f, "Documents"),
            FormatCategory::Spreadsheet => write!(f, "Spreadsheets"),
            FormatCategory::Data => write!(f, "Data"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    // Images
    Svg,
    Png,
    Jpg,
    Webp,
    Ico,
    Bmp,
    Tiff,
    Gif,

    // Video
    Mp4,
    Mov,
    Mkv,
    Avi,
    Webm,

    // Audio
    Mp3,
    Wav,
    Flac,
    Aac,
    M4a,
    Ogg,

    // Spreadsheets & Data
    Csv,
    Xlsx,
    Json,
    Tsv,

    // Documents
    Pdf,
    Docx,
    Txt,
    Html,
}

impl FileFormat {
    /// Attempt to identify the file format by its file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean.as_str() {
            // Images
            "svg" => Some(FileFormat::Svg),
            "png" => Some(FileFormat::Png),
            "jpg" | "jpeg" | "jpe" | "jfif" => Some(FileFormat::Jpg),
            "webp" => Some(FileFormat::Webp),
            "ico" => Some(FileFormat::Ico),
            "bmp" => Some(FileFormat::Bmp),
            "tif" | "tiff" => Some(FileFormat::Tiff),
            "gif" => Some(FileFormat::Gif),

            // Video
            "mp4" | "m4v" => Some(FileFormat::Mp4),
            "mov" => Some(FileFormat::Mov),
            "mkv" => Some(FileFormat::Mkv),
            "avi" => Some(FileFormat::Avi),
            "webm" => Some(FileFormat::Webm),

            // Audio
            "mp3" => Some(FileFormat::Mp3),
            "wav" => Some(FileFormat::Wav),
            "flac" => Some(FileFormat::Flac),
            "aac" => Some(FileFormat::Aac),
            "m4a" => Some(FileFormat::M4a),
            "ogg" | "oga" => Some(FileFormat::Ogg),

            // Spreadsheets & Data
            "csv" => Some(FileFormat::Csv),
            "xlsx" => Some(FileFormat::Xlsx),
            "json" => Some(FileFormat::Json),
            "tsv" => Some(FileFormat::Tsv),

            // Documents
            "pdf" => Some(FileFormat::Pdf),
            "docx" => Some(FileFormat::Docx),
            "txt" => Some(FileFormat::Txt),
            "html" | "htm" => Some(FileFormat::Html),

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
            FileFormat::Gif => "gif",

            FileFormat::Mp4 => "mp4",
            FileFormat::Mov => "mov",
            FileFormat::Mkv => "mkv",
            FileFormat::Avi => "avi",
            FileFormat::Webm => "webm",

            FileFormat::Mp3 => "mp3",
            FileFormat::Wav => "wav",
            FileFormat::Flac => "flac",
            FileFormat::Aac => "aac",
            FileFormat::M4a => "m4a",
            FileFormat::Ogg => "ogg",

            FileFormat::Csv => "csv",
            FileFormat::Xlsx => "xlsx",
            FileFormat::Json => "json",
            FileFormat::Tsv => "tsv",

            FileFormat::Pdf => "pdf",
            FileFormat::Docx => "docx",
            FileFormat::Txt => "txt",
            FileFormat::Html => "html",
        }
    }

    /// Human-readable label (e.g. for radial wheel badge).
    pub fn display_label(&self) -> &'static str {
        match self {
            FileFormat::Svg => "SVG",
            FileFormat::Png => "PNG",
            FileFormat::Jpg => "JPG",
            FileFormat::Webp => "WEBP",
            FileFormat::Ico => "ICO",
            FileFormat::Bmp => "BMP",
            FileFormat::Tiff => "TIFF",
            FileFormat::Gif => "GIF",

            FileFormat::Mp4 => "MP4",
            FileFormat::Mov => "MOV",
            FileFormat::Mkv => "MKV",
            FileFormat::Avi => "AVI",
            FileFormat::Webm => "WEBM",

            FileFormat::Mp3 => "MP3",
            FileFormat::Wav => "WAV",
            FileFormat::Flac => "FLAC",
            FileFormat::Aac => "AAC",
            FileFormat::M4a => "M4A",
            FileFormat::Ogg => "OGG",

            FileFormat::Csv => "CSV",
            FileFormat::Xlsx => "XLSX",
            FileFormat::Json => "JSON",
            FileFormat::Tsv => "TSV",

            FileFormat::Pdf => "PDF",
            FileFormat::Docx => "DOCX",
            FileFormat::Txt => "TXT",
            FileFormat::Html => "HTML",
        }
    }

    /// Category for grouping in UI and capability matrix.
    pub fn category(&self) -> FormatCategory {
        match self {
            FileFormat::Svg
            | FileFormat::Png
            | FileFormat::Jpg
            | FileFormat::Webp
            | FileFormat::Ico
            | FileFormat::Bmp
            | FileFormat::Tiff
            | FileFormat::Gif => FormatCategory::Image,

            FileFormat::Mp4
            | FileFormat::Mov
            | FileFormat::Mkv
            | FileFormat::Avi
            | FileFormat::Webm => FormatCategory::Video,

            FileFormat::Mp3
            | FileFormat::Wav
            | FileFormat::Flac
            | FileFormat::Aac
            | FileFormat::M4a
            | FileFormat::Ogg => FormatCategory::Audio,

            FileFormat::Csv
            | FileFormat::Xlsx
            | FileFormat::Tsv => FormatCategory::Spreadsheet,

            FileFormat::Json => FormatCategory::Data,

            FileFormat::Pdf
            | FileFormat::Docx
            | FileFormat::Txt
            | FileFormat::Html => FormatCategory::Document,
        }
    }

    pub fn is_image(&self) -> bool {
        self.category() == FormatCategory::Image
    }

    pub fn is_video(&self) -> bool {
        self.category() == FormatCategory::Video
    }

    pub fn is_audio(&self) -> bool {
        self.category() == FormatCategory::Audio
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
            FileFormat::Gif => "image/gif",

            FileFormat::Mp4 => "video/mp4",
            FileFormat::Mov => "video/quicktime",
            FileFormat::Mkv => "video/x-matroska",
            FileFormat::Avi => "video/x-msvideo",
            FileFormat::Webm => "video/webm",

            FileFormat::Mp3 => "audio/mpeg",
            FileFormat::Wav => "audio/wav",
            FileFormat::Flac => "audio/flac",
            FileFormat::Aac => "audio/aac",
            FileFormat::M4a => "audio/mp4",
            FileFormat::Ogg => "audio/ogg",

            FileFormat::Csv => "text/csv",
            FileFormat::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            FileFormat::Json => "application/json",
            FileFormat::Tsv => "text/tab-separated-values",

            FileFormat::Pdf => "application/pdf",
            FileFormat::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            FileFormat::Txt => "text/plain",
            FileFormat::Html => "text/html",
        }
    }

    /// Whether this format inherently supports alpha transparency.
    pub fn supports_transparency(&self) -> bool {
        match self {
            FileFormat::Svg | FileFormat::Png | FileFormat::Webp | FileFormat::Ico | FileFormat::Gif => true,
            _ => false,
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
        assert_eq!(FileFormat::from_extension("SVG"), Some(FileFormat::Svg));
        assert_eq!(FileFormat::from_extension("mp4"), Some(FileFormat::Mp4));
        assert_eq!(FileFormat::from_extension("mp3"), Some(FileFormat::Mp3));
        assert_eq!(FileFormat::from_extension("csv"), Some(FileFormat::Csv));
        assert_eq!(FileFormat::from_extension("xyz123"), None);
    }

    #[test]
    fn test_format_from_path() {
        let p = Path::new("C:/test/file.WEBP");
        assert_eq!(FileFormat::from_path(p), Some(FileFormat::Webp));
        let p2 = Path::new("C:/test/sample.svg");
        assert_eq!(FileFormat::from_path(p2), Some(FileFormat::Svg));
        let p3 = Path::new("C:/test/video.mp4");
        assert_eq!(FileFormat::from_path(p3), Some(FileFormat::Mp4));
    }
}

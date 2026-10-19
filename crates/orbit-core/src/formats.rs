use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FormatCategory {
    Raster,
    ModernWeb,
    Vector,
    CameraRaw,
    Specialized,
    Video,
    Audio,
    Document,
    Spreadsheet,
    Data,
}

impl fmt::Display for FormatCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatCategory::Raster => write!(f, "Raster Images"),
            FormatCategory::ModernWeb => write!(f, "Modern Web Formats"),
            FormatCategory::Vector => write!(f, "Vector & Graphics"),
            FormatCategory::CameraRaw => write!(f, "Camera RAW"),
            FormatCategory::Specialized => write!(f, "Specialized & Archives"),
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
    // Standard Raster
    Png,
    Jpg,
    Webp,
    Ico,
    Bmp,
    Tiff,
    Gif,
    Tga,
    Dds,
    Ppm,
    Pcx,
    Dpx,

    // Modern Web / Next-Gen
    Avif,
    Heic,
    Jxl,

    // Vector & Graphics
    Svg,
    Eps,
    Art,
    Dcs,
    Wmz,
    Emz,

    // Camera RAW
    Cr2,
    Cr3,
    Crw,
    Nef,
    Nrw,
    Arw,
    Sr2,
    Srf,
    Raf,
    Orf,
    Rw2,
    Dng,
    Pef,
    Ptx,
    Rwl,
    Srw,
    X3f,
    Dcr,
    Drf,
    Kdc,
    K25,

    // Specialized & Archives
    Psd,
    Psb,
    Djvu,
    Cbr,
    Cbz,
    Odd,

    // Video
    Mp4,
    Mov,
    Mkv,
    Avi,
    Webm,
    Wmv,
    Asf,
    Flv,
    Mpeg,
    M4v,
    Ts,
    Mts,
    Vob,
    Mxf,
    ThreeGp,
    ThreeG2,
    Ogv,

    // Audio
    Mp3,
    Wav,
    Flac,
    Aac,
    M4a,
    Ogg,
    Opus,

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
    /// Attempt to identify the file format by its file extension with alias normalization.
    /// Camera manufacturer names are never treated as file extensions.
    pub fn from_extension(ext: &str) -> Option<Self> {
        let clean = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean.as_str() {
            // Standard Raster
            "png" => Some(FileFormat::Png),
            "jpg" | "jpeg" | "jpe" | "jfif" => Some(FileFormat::Jpg),
            "webp" => Some(FileFormat::Webp),
            "ico" => Some(FileFormat::Ico),
            "bmp" | "dib" => Some(FileFormat::Bmp),
            "tif" | "tiff" => Some(FileFormat::Tiff),
            "gif" => Some(FileFormat::Gif),
            "tga" | "icb" | "vda" | "vst" => Some(FileFormat::Tga),
            "dds" => Some(FileFormat::Dds),
            "ppm" | "pgm" | "pbm" | "pnm" => Some(FileFormat::Ppm),
            "pcx" => Some(FileFormat::Pcx),
            "dpx" => Some(FileFormat::Dpx),

            // Modern Web
            "avif" | "avis" => Some(FileFormat::Avif),
            "heic" | "heif" | "heics" | "heifs" => Some(FileFormat::Heic),
            "jxl" => Some(FileFormat::Jxl),

            // Vector & Graphics
            "svg" | "svgz" => Some(FileFormat::Svg),
            "eps" | "epi" | "epsf" | "epsi" => Some(FileFormat::Eps),
            "art" => Some(FileFormat::Art),
            "dcs" => Some(FileFormat::Dcs),
            "wmz" => Some(FileFormat::Wmz),
            "emz" => Some(FileFormat::Emz),

            // Camera RAW
            "cr2" => Some(FileFormat::Cr2),
            "cr3" => Some(FileFormat::Cr3),
            "crw" => Some(FileFormat::Crw),
            "nef" => Some(FileFormat::Nef),
            "nrw" => Some(FileFormat::Nrw),
            "arw" => Some(FileFormat::Arw),
            "sr2" => Some(FileFormat::Sr2),
            "srf" => Some(FileFormat::Srf),
            "raf" => Some(FileFormat::Raf),
            "orf" => Some(FileFormat::Orf),
            "rw2" => Some(FileFormat::Rw2),
            "dng" => Some(FileFormat::Dng),
            "pef" => Some(FileFormat::Pef),
            "ptx" => Some(FileFormat::Ptx),
            "rwl" => Some(FileFormat::Rwl),
            "srw" => Some(FileFormat::Srw),
            "x3f" => Some(FileFormat::X3f),
            "dcr" => Some(FileFormat::Dcr),
            "drf" => Some(FileFormat::Drf),
            "kdc" => Some(FileFormat::Kdc),
            "k25" => Some(FileFormat::K25),

            // Specialized & Archives
            "psd" => Some(FileFormat::Psd),
            "psb" => Some(FileFormat::Psb),
            "djv" | "djvu" => Some(FileFormat::Djvu),
            "cbr" => Some(FileFormat::Cbr),
            "cbz" => Some(FileFormat::Cbz),
            "odd" => Some(FileFormat::Odd),

            // Video
            "mp4" => Some(FileFormat::Mp4),
            "m4v" => Some(FileFormat::M4v),
            "mov" | "qt" => Some(FileFormat::Mov),
            "mkv" => Some(FileFormat::Mkv),
            "avi" => Some(FileFormat::Avi),
            "webm" => Some(FileFormat::Webm),
            "wmv" => Some(FileFormat::Wmv),
            "asf" => Some(FileFormat::Asf),
            "flv" | "f4v" => Some(FileFormat::Flv),
            "mpg" | "mpeg" | "mpe" | "m2v" => Some(FileFormat::Mpeg),
            "ts" => Some(FileFormat::Ts),
            "mts" | "m2ts" => Some(FileFormat::Mts),
            "vob" => Some(FileFormat::Vob),
            "mxf" => Some(FileFormat::Mxf),
            "3gp" | "3gpp" => Some(FileFormat::ThreeGp),
            "3g2" => Some(FileFormat::ThreeG2),
            "ogv" => Some(FileFormat::Ogv),

            // Audio
            "mp3" => Some(FileFormat::Mp3),
            "wav" => Some(FileFormat::Wav),
            "flac" => Some(FileFormat::Flac),
            "aac" => Some(FileFormat::Aac),
            "m4a" => Some(FileFormat::M4a),
            "ogg" | "oga" => Some(FileFormat::Ogg),
            "opus" => Some(FileFormat::Opus),

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

    /// Sniff format from the initial bytes (magic numbers) of a file.
    pub fn sniff_magic_bytes(header: &[u8]) -> Option<Self> {
        if header.len() < 4 {
            return None;
        }

        // PNG: 89 50 4E 47
        if header.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
            return Some(FileFormat::Png);
        }

        // JPEG: FF D8 FF
        if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(FileFormat::Jpg);
        }

        // GIF: GIF87a or GIF89a
        if header.starts_with(b"GIF87a") || header.starts_with(b"GIF89a") {
            return Some(FileFormat::Gif);
        }

        // BMP: 42 4D ('BM')
        if header.starts_with(b"BM") {
            return Some(FileFormat::Bmp);
        }

        // TIFF (LE or BE) / Canon CR2 / DNG
        if header.starts_with(&[0x49, 0x49, 0x2A, 0x00]) {
            // Check Canon CR2 specific offset if available
            if header.len() >= 10 && header[8..10] == [0x43, 0x52] {
                return Some(FileFormat::Cr2);
            }
            return Some(FileFormat::Tiff);
        }
        if header.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
            return Some(FileFormat::Tiff);
        }

        // WebP: RIFF .... WEBP
        if header.len() >= 12 && header.starts_with(b"RIFF") && &header[8..12] == b"WEBP" {
            return Some(FileFormat::Webp);
        }

        // ICO: 00 00 01 00
        if header.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
            return Some(FileFormat::Ico);
        }

        // PSD / PSB: 8BPS
        if header.starts_with(b"8BPS") {
            if header.len() >= 6 && header[4..6] == [0x00, 0x02] {
                return Some(FileFormat::Psb);
            }
            return Some(FileFormat::Psd);
        }

        // Fujifilm RAF: FUJIFILMCCD
        if header.starts_with(b"FUJIFILM") {
            return Some(FileFormat::Raf);
        }

        // PDF: %PDF
        if header.starts_with(b"%PDF") {
            return Some(FileFormat::Pdf);
        }

        // MP4 / MOV / M4V / 3GP / 3G2: ftyp or moov or wide
        if header.len() >= 8 {
            let magic4 = &header[4..8];
            if magic4 == b"ftyp" || magic4 == b"moov" || magic4 == b"wide" {
                if header.len() >= 12 && &header[8..12] == b"qt  " {
                    return Some(FileFormat::Mov);
                }
                return Some(FileFormat::Mp4);
            }
        }

        // Matroska / WebM: EBML ID 1A 45 DF A3
        if header.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            return Some(FileFormat::Mkv);
        }

        // AVI: RIFF .... AVI 
        if header.len() >= 12 && header.starts_with(b"RIFF") && &header[8..12] == b"AVI " {
            return Some(FileFormat::Avi);
        }

        // Flash Video: FLV
        if header.starts_with(b"FLV") {
            return Some(FileFormat::Flv);
        }

        // ASF / WMV: 30 26 B2 75 8E 66 CF 11
        if header.starts_with(&[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11]) {
            return Some(FileFormat::Wmv);
        }

        // MPEG-PS (VOB / MPG): 00 00 01 BA
        if header.starts_with(&[0x00, 0x00, 0x01, 0xBA]) {
            return Some(FileFormat::Mpeg);
        }

        // MPEG-TS: Sync byte 0x47
        if header.starts_with(&[0x47]) {
            return Some(FileFormat::Ts);
        }

        // OGG / OGV / OPUS: OggS
        if header.starts_with(b"OggS") {
            return Some(FileFormat::Ogg);
        }

        None
    }

    /// Identify the format from a file path (checking extension, then optionally fallback).
    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<Self> {
        let p = path.as_ref();
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if let Some(fmt) = Self::from_extension(ext) {
                return Some(fmt);
            }
        }

        // If extension lookup failed or file has ambiguous extension, try reading first 32 bytes
        if let Ok(mut f) = std::fs::File::open(p) {
            use std::io::Read;
            let mut buf = [0u8; 32];
            if let Ok(n) = f.read(&mut buf) {
                if n >= 4 {
                    if let Some(fmt) = Self::sniff_magic_bytes(&buf[..n]) {
                        return Some(fmt);
                    }
                }
            }
        }

        None
    }

    /// Primary file extension for this format without leading dot.
    pub fn primary_extension(&self) -> &'static str {
        match self {
            FileFormat::Png => "png",
            FileFormat::Jpg => "jpg",
            FileFormat::Webp => "webp",
            FileFormat::Ico => "ico",
            FileFormat::Bmp => "bmp",
            FileFormat::Tiff => "tiff",
            FileFormat::Gif => "gif",
            FileFormat::Tga => "tga",
            FileFormat::Dds => "dds",
            FileFormat::Ppm => "ppm",
            FileFormat::Pcx => "pcx",
            FileFormat::Dpx => "dpx",

            FileFormat::Avif => "avif",
            FileFormat::Heic => "heic",
            FileFormat::Jxl => "jxl",

            FileFormat::Svg => "svg",
            FileFormat::Eps => "eps",
            FileFormat::Art => "art",
            FileFormat::Dcs => "dcs",
            FileFormat::Wmz => "wmz",
            FileFormat::Emz => "emz",

            FileFormat::Cr2 => "cr2",
            FileFormat::Cr3 => "cr3",
            FileFormat::Crw => "crw",
            FileFormat::Nef => "nef",
            FileFormat::Nrw => "nrw",
            FileFormat::Arw => "arw",
            FileFormat::Sr2 => "sr2",
            FileFormat::Srf => "srf",
            FileFormat::Raf => "raf",
            FileFormat::Orf => "orf",
            FileFormat::Rw2 => "rw2",
            FileFormat::Dng => "dng",
            FileFormat::Pef => "pef",
            FileFormat::Ptx => "ptx",
            FileFormat::Rwl => "rwl",
            FileFormat::Srw => "srw",
            FileFormat::X3f => "x3f",
            FileFormat::Dcr => "dcr",
            FileFormat::Drf => "drf",
            FileFormat::Kdc => "kdc",
            FileFormat::K25 => "k25",

            FileFormat::Psd => "psd",
            FileFormat::Psb => "psb",
            FileFormat::Djvu => "djvu",
            FileFormat::Cbr => "cbr",
            FileFormat::Cbz => "cbz",
            FileFormat::Odd => "odd",

            FileFormat::Mp4 => "mp4",
            FileFormat::Mov => "mov",
            FileFormat::Mkv => "mkv",
            FileFormat::Avi => "avi",
            FileFormat::Webm => "webm",
            FileFormat::Wmv => "wmv",
            FileFormat::Asf => "asf",
            FileFormat::Flv => "flv",
            FileFormat::Mpeg => "mpg",
            FileFormat::M4v => "m4v",
            FileFormat::Ts => "ts",
            FileFormat::Mts => "mts",
            FileFormat::Vob => "vob",
            FileFormat::Mxf => "mxf",
            FileFormat::ThreeGp => "3gp",
            FileFormat::ThreeG2 => "3g2",
            FileFormat::Ogv => "ogv",

            FileFormat::Mp3 => "mp3",
            FileFormat::Wav => "wav",
            FileFormat::Flac => "flac",
            FileFormat::Aac => "aac",
            FileFormat::M4a => "m4a",
            FileFormat::Ogg => "ogg",
            FileFormat::Opus => "opus",

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
            FileFormat::Png => "PNG",
            FileFormat::Jpg => "JPG",
            FileFormat::Webp => "WEBP",
            FileFormat::Ico => "ICO",
            FileFormat::Bmp => "BMP",
            FileFormat::Tiff => "TIFF",
            FileFormat::Gif => "GIF",
            FileFormat::Tga => "TGA",
            FileFormat::Dds => "DDS",
            FileFormat::Ppm => "PPM",
            FileFormat::Pcx => "PCX",
            FileFormat::Dpx => "DPX",

            FileFormat::Avif => "AVIF",
            FileFormat::Heic => "HEIC",
            FileFormat::Jxl => "JXL",

            FileFormat::Svg => "SVG",
            FileFormat::Eps => "EPS",
            FileFormat::Art => "ART",
            FileFormat::Dcs => "DCS",
            FileFormat::Wmz => "WMZ",
            FileFormat::Emz => "EMZ",

            FileFormat::Cr2 => "CR2",
            FileFormat::Cr3 => "CR3",
            FileFormat::Crw => "CRW",
            FileFormat::Nef => "NEF",
            FileFormat::Nrw => "NRW",
            FileFormat::Arw => "ARW",
            FileFormat::Sr2 => "SR2",
            FileFormat::Srf => "SRF",
            FileFormat::Raf => "RAF",
            FileFormat::Orf => "ORF",
            FileFormat::Rw2 => "RW2",
            FileFormat::Dng => "DNG",
            FileFormat::Pef => "PEF",
            FileFormat::Ptx => "PTX",
            FileFormat::Rwl => "RWL",
            FileFormat::Srw => "SRW",
            FileFormat::X3f => "X3F",
            FileFormat::Dcr => "DCR",
            FileFormat::Drf => "DRF",
            FileFormat::Kdc => "KDC",
            FileFormat::K25 => "K25",

            FileFormat::Psd => "PSD",
            FileFormat::Psb => "PSB",
            FileFormat::Djvu => "DJVU",
            FileFormat::Cbr => "CBR",
            FileFormat::Cbz => "CBZ",
            FileFormat::Odd => "ODD",

            FileFormat::Mp4 => "MP4",
            FileFormat::Mov => "MOV",
            FileFormat::Mkv => "MKV",
            FileFormat::Avi => "AVI",
            FileFormat::Webm => "WEBM",
            FileFormat::Wmv => "WMV",
            FileFormat::Asf => "ASF",
            FileFormat::Flv => "FLV",
            FileFormat::Mpeg => "MPEG",
            FileFormat::M4v => "M4V",
            FileFormat::Ts => "TS",
            FileFormat::Mts => "MTS",
            FileFormat::Vob => "VOB",
            FileFormat::Mxf => "MXF",
            FileFormat::ThreeGp => "3GP",
            FileFormat::ThreeG2 => "3G2",
            FileFormat::Ogv => "OGV",

            FileFormat::Mp3 => "MP3",
            FileFormat::Wav => "WAV",
            FileFormat::Flac => "FLAC",
            FileFormat::Aac => "AAC",
            FileFormat::M4a => "M4A",
            FileFormat::Ogg => "OGG",
            FileFormat::Opus => "OPUS",

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
            FileFormat::Png
            | FileFormat::Jpg
            | FileFormat::Webp
            | FileFormat::Ico
            | FileFormat::Bmp
            | FileFormat::Tiff
            | FileFormat::Gif
            | FileFormat::Tga
            | FileFormat::Dds
            | FileFormat::Ppm
            | FileFormat::Pcx
            | FileFormat::Dpx => FormatCategory::Raster,

            FileFormat::Avif | FileFormat::Heic | FileFormat::Jxl => FormatCategory::ModernWeb,

            FileFormat::Svg
            | FileFormat::Eps
            | FileFormat::Art
            | FileFormat::Dcs
            | FileFormat::Wmz
            | FileFormat::Emz => FormatCategory::Vector,

            FileFormat::Cr2
            | FileFormat::Cr3
            | FileFormat::Crw
            | FileFormat::Nef
            | FileFormat::Nrw
            | FileFormat::Arw
            | FileFormat::Sr2
            | FileFormat::Srf
            | FileFormat::Raf
            | FileFormat::Orf
            | FileFormat::Rw2
            | FileFormat::Dng
            | FileFormat::Pef
            | FileFormat::Ptx
            | FileFormat::Rwl
            | FileFormat::Srw
            | FileFormat::X3f
            | FileFormat::Dcr
            | FileFormat::Drf
            | FileFormat::Kdc
            | FileFormat::K25 => FormatCategory::CameraRaw,

            FileFormat::Psd
            | FileFormat::Psb
            | FileFormat::Djvu
            | FileFormat::Cbr
            | FileFormat::Cbz
            | FileFormat::Odd => FormatCategory::Specialized,

            FileFormat::Mp4
            | FileFormat::Mov
            | FileFormat::Mkv
            | FileFormat::Avi
            | FileFormat::Webm
            | FileFormat::Wmv
            | FileFormat::Asf
            | FileFormat::Flv
            | FileFormat::Mpeg
            | FileFormat::M4v
            | FileFormat::Ts
            | FileFormat::Mts
            | FileFormat::Vob
            | FileFormat::Mxf
            | FileFormat::ThreeGp
            | FileFormat::ThreeG2
            | FileFormat::Ogv => FormatCategory::Video,

            FileFormat::Mp3
            | FileFormat::Wav
            | FileFormat::Flac
            | FileFormat::Aac
            | FileFormat::M4a
            | FileFormat::Ogg
            | FileFormat::Opus => FormatCategory::Audio,

            FileFormat::Csv | FileFormat::Xlsx | FileFormat::Tsv => FormatCategory::Spreadsheet,

            FileFormat::Json => FormatCategory::Data,

            FileFormat::Pdf | FileFormat::Docx | FileFormat::Txt | FileFormat::Html => {
                FormatCategory::Document
            }
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(
            self.category(),
            FormatCategory::Raster
                | FormatCategory::ModernWeb
                | FormatCategory::Vector
                | FormatCategory::CameraRaw
                | FormatCategory::Specialized
        )
    }

    pub fn is_camera_raw(&self) -> bool {
        self.category() == FormatCategory::CameraRaw
    }

    pub fn is_video(&self) -> bool {
        self.category() == FormatCategory::Video
    }

    pub fn is_audio(&self) -> bool {
        self.category() == FormatCategory::Audio
    }

    /// Whether this format inherently supports alpha transparency.
    pub fn supports_transparency(&self) -> bool {
        matches!(
            self,
            FileFormat::Svg
                | FileFormat::Png
                | FileFormat::Webp
                | FileFormat::Ico
                | FileFormat::Gif
                | FileFormat::Tga
                | FileFormat::Tiff
                | FileFormat::Avif
                | FileFormat::Heic
                | FileFormat::Jxl
                | FileFormat::Psd
        )
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
        assert_eq!(FileFormat::from_extension("jfif"), Some(FileFormat::Jpg));
        assert_eq!(FileFormat::from_extension("SVG"), Some(FileFormat::Svg));
        assert_eq!(FileFormat::from_extension("mp4"), Some(FileFormat::Mp4));
        assert_eq!(FileFormat::from_extension("mp3"), Some(FileFormat::Mp3));
        assert_eq!(FileFormat::from_extension("csv"), Some(FileFormat::Csv));
        assert_eq!(FileFormat::from_extension("cr2"), Some(FileFormat::Cr2));
        assert_eq!(FileFormat::from_extension("nef"), Some(FileFormat::Nef));
        assert_eq!(FileFormat::from_extension("arw"), Some(FileFormat::Arw));
        assert_eq!(FileFormat::from_extension("dng"), Some(FileFormat::Dng));
        assert_eq!(FileFormat::from_extension("avif"), Some(FileFormat::Avif));
        assert_eq!(FileFormat::from_extension("heic"), Some(FileFormat::Heic));
        assert_eq!(FileFormat::from_extension("jxl"), Some(FileFormat::Jxl));
        assert_eq!(FileFormat::from_extension("psd"), Some(FileFormat::Psd));
        assert_eq!(FileFormat::from_extension("cbz"), Some(FileFormat::Cbz));
        assert_eq!(FileFormat::from_extension("canon"), None);
        assert_eq!(FileFormat::from_extension("nikon"), None);
        assert_eq!(FileFormat::from_extension("sony"), None);
    }

    #[test]
    fn test_format_from_path() {
        let p = Path::new("C:/test/file.WEBP");
        assert_eq!(FileFormat::from_path(p), Some(FileFormat::Webp));
        let p2 = Path::new("C:/test/sample.svg");
        assert_eq!(FileFormat::from_path(p2), Some(FileFormat::Svg));
        let p3 = Path::new("C:/test/raw_photo.CR2");
        assert_eq!(FileFormat::from_path(p3), Some(FileFormat::Cr2));
    }

    #[test]
    fn test_magic_bytes_sniffing() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(FileFormat::sniff_magic_bytes(&png_header), Some(FileFormat::Png));

        let jpg_header = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(FileFormat::sniff_magic_bytes(&jpg_header), Some(FileFormat::Jpg));

        let gif_header = b"GIF89a...";
        assert_eq!(FileFormat::sniff_magic_bytes(gif_header), Some(FileFormat::Gif));
    }
}

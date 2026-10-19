use crate::formats::FileFormat;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuickAction {
    // Images
    RemoveBackground,
    ResizeImage,
    CompressImage,
    UpscaleImage,
    CropImage,
    RotateImage,
    RemoveMetadata,
    ConvertToIcon,
    OptimizeImage,
    BatchProcess,

    // Videos
    CompressVideo,
    ExtractAudio,
    RemoveAudio,
    TrimVideo,
    ResizeVideo,
    ChangeFps,
    ExtractFrames,
    GenerateThumbnail,

    // Audio
    NormalizeVolume,
    TrimAudio,
    CompressAudio,
    ExtractAudioMetadata,
    RemoveAudioMetadata,
    ChangeSampleRate,
    ConvertToMono,
    ConvertToStereo,

    // Documents
    ConvertToPdf,
    ExtractText,
    OcrDocument,
    MergeDocuments,
    SplitDocument,
    CompressDocument,
    PasswordProtect,
    RemovePassword,
    FlattenDocument,

    // Archives
    ExtractHere,
    ExtractToFolder,
    ListContents,
    CompressFurther,
    ConvertTo7z,
    ConvertToZip,
    TestIntegrity,

    // 3D Models
    ConvertToGlb,
    OptimizeGeometry,
    Generate3dThumbnail,
    View3d,
    DecimateMesh,
    ExtractTextures,
    ConvertToStl,

    // Generic / Fallback
    CopyPath,
    ShowInExplorer,
    CalculateHash,
}

impl QuickAction {
    pub fn id(&self) -> &'static str {
        match self {
            QuickAction::RemoveBackground => "remove_bg",
            QuickAction::ResizeImage => "resize_img",
            QuickAction::CompressImage => "compress_img",
            QuickAction::UpscaleImage => "upscale_img",
            QuickAction::CropImage => "crop_img",
            QuickAction::RotateImage => "rotate_img",
            QuickAction::RemoveMetadata => "remove_meta",
            QuickAction::ConvertToIcon => "to_icon",
            QuickAction::OptimizeImage => "optimize_img",
            QuickAction::BatchProcess => "batch_proc",

            QuickAction::CompressVideo => "compress_vid",
            QuickAction::ExtractAudio => "extract_audio",
            QuickAction::RemoveAudio => "mute_vid",
            QuickAction::TrimVideo => "trim_vid",
            QuickAction::ResizeVideo => "resize_vid",
            QuickAction::ChangeFps => "fps_vid",
            QuickAction::ExtractFrames => "frames_vid",
            QuickAction::GenerateThumbnail => "thumb_vid",

            QuickAction::NormalizeVolume => "norm_vol",
            QuickAction::TrimAudio => "trim_aud",
            QuickAction::CompressAudio => "compress_aud",
            QuickAction::ExtractAudioMetadata => "meta_aud",
            QuickAction::RemoveAudioMetadata => "strip_aud",
            QuickAction::ChangeSampleRate => "rate_aud",
            QuickAction::ConvertToMono => "to_mono",
            QuickAction::ConvertToStereo => "to_stereo",

            QuickAction::ConvertToPdf => "to_pdf",
            QuickAction::ExtractText => "get_text",
            QuickAction::OcrDocument => "ocr_doc",
            QuickAction::MergeDocuments => "merge_doc",
            QuickAction::SplitDocument => "split_doc",
            QuickAction::CompressDocument => "compress_doc",
            QuickAction::PasswordProtect => "protect_doc",
            QuickAction::RemovePassword => "unprotect_doc",
            QuickAction::FlattenDocument => "flatten_doc",

            QuickAction::ExtractHere => "extract_here",
            QuickAction::ExtractToFolder => "extract_folder",
            QuickAction::ListContents => "list_arch",
            QuickAction::CompressFurther => "ultra_arch",
            QuickAction::ConvertTo7z => "to_7z",
            QuickAction::ConvertToZip => "to_zip",
            QuickAction::TestIntegrity => "test_arch",

            QuickAction::ConvertToGlb => "to_glb",
            QuickAction::OptimizeGeometry => "opt_geom",
            QuickAction::Generate3dThumbnail => "thumb_3d",
            QuickAction::View3d => "view_3d",
            QuickAction::DecimateMesh => "decimate_3d",
            QuickAction::ExtractTextures => "tex_3d",
            QuickAction::ConvertToStl => "to_stl",

            QuickAction::CopyPath => "copy_path",
            QuickAction::ShowInExplorer => "open_folder",
            QuickAction::CalculateHash => "sha256",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            QuickAction::RemoveBackground => "Remove BG",
            QuickAction::ResizeImage => "Resize",
            QuickAction::CompressImage => "Compress",
            QuickAction::UpscaleImage => "Upscale",
            QuickAction::CropImage => "Crop",
            QuickAction::RotateImage => "Rotate 90°",
            QuickAction::RemoveMetadata => "Strip Exif",
            QuickAction::ConvertToIcon => "Make ICO",
            QuickAction::OptimizeImage => "Optimize",
            QuickAction::BatchProcess => "Batch",

            QuickAction::CompressVideo => "Compress",
            QuickAction::ExtractAudio => "Extract MP3",
            QuickAction::RemoveAudio => "Mute Video",
            QuickAction::TrimVideo => "Trim",
            QuickAction::ResizeVideo => "Resize 1080p",
            QuickAction::ChangeFps => "60 FPS",
            QuickAction::ExtractFrames => "Frames",
            QuickAction::GenerateThumbnail => "Thumbnail",

            QuickAction::NormalizeVolume => "Normalize",
            QuickAction::TrimAudio => "Trim",
            QuickAction::CompressAudio => "Compress 128k",
            QuickAction::ExtractAudioMetadata => "Inspect",
            QuickAction::RemoveAudioMetadata => "Strip Exif",
            QuickAction::ChangeSampleRate => "44.1 kHz",
            QuickAction::ConvertToMono => "To Mono",
            QuickAction::ConvertToStereo => "To Stereo",

            QuickAction::ConvertToPdf => "To PDF",
            QuickAction::ExtractText => "Extract Text",
            QuickAction::OcrDocument => "OCR Read",
            QuickAction::MergeDocuments => "Merge",
            QuickAction::SplitDocument => "Split",
            QuickAction::CompressDocument => "Compress",
            QuickAction::PasswordProtect => "Protect",
            QuickAction::RemovePassword => "Unlock",
            QuickAction::FlattenDocument => "Flatten",

            QuickAction::ExtractHere => "Extract Here",
            QuickAction::ExtractToFolder => "To Folder",
            QuickAction::ListContents => "List Files",
            QuickAction::CompressFurther => "Ultra Zip",
            QuickAction::ConvertTo7z => "To 7Z",
            QuickAction::ConvertToZip => "To ZIP",
            QuickAction::TestIntegrity => "Test Arch",

            QuickAction::ConvertToGlb => "To GLB",
            QuickAction::OptimizeGeometry => "Optimize",
            QuickAction::Generate3dThumbnail => "Thumbnail",
            QuickAction::View3d => "View 3D",
            QuickAction::DecimateMesh => "Decimate",
            QuickAction::ExtractTextures => "Textures",
            QuickAction::ConvertToStl => "To STL",

            QuickAction::CopyPath => "Copy Path",
            QuickAction::ShowInExplorer => "Show Folder",
            QuickAction::CalculateHash => "SHA-256",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            QuickAction::RemoveBackground => "AI background eraser",
            QuickAction::ResizeImage => "Custom resolution or scale",
            QuickAction::CompressImage => "Lossless or target size",
            QuickAction::UpscaleImage => "Super resolution upscaling",
            QuickAction::CropImage => "Interactive bounds cropping",
            QuickAction::RotateImage => "Rotate 90 degrees clockwise",
            QuickAction::RemoveMetadata => "Strip GPS & EXIF privacy",
            QuickAction::ConvertToIcon => "Multi-res Windows .ico",
            QuickAction::OptimizeImage => "Web optimization pass",
            QuickAction::BatchProcess => "Apply to sibling files",

            QuickAction::CompressVideo => "CRF compression pass",
            QuickAction::ExtractAudio => "Extract 320kbps audio track",
            QuickAction::RemoveAudio => "Mute video audio streams",
            QuickAction::TrimVideo => "Cut beginning or end segment",
            QuickAction::ResizeVideo => "Downscale to 1080p Full HD",
            QuickAction::ChangeFps => "Resample to 60fps smooth",
            QuickAction::ExtractFrames => "Export keyframes as PNGs",
            QuickAction::GenerateThumbnail => "Capture high-res cover",

            QuickAction::NormalizeVolume => "EBU R128 loudness balance",
            QuickAction::TrimAudio => "Cut start or trailing silence",
            QuickAction::CompressAudio => "Compact 128kbps encode",
            QuickAction::ExtractAudioMetadata => "Read ID3 tags & bitrate",
            QuickAction::RemoveAudioMetadata => "Remove ID3 tags and art",
            QuickAction::ChangeSampleRate => "Resample audio clock",
            QuickAction::ConvertToMono => "Downmix to single channel",
            QuickAction::ConvertToStereo => "Upmix or dual channel",

            QuickAction::ConvertToPdf => "Compile document to PDF",
            QuickAction::ExtractText => "Extract readable text dump",
            QuickAction::OcrDocument => "Local optical character recognition",
            QuickAction::MergeDocuments => "Combine pages with next file",
            QuickAction::SplitDocument => "Split into individual pages",
            QuickAction::CompressDocument => "Reduce PDF raster image sizes",
            QuickAction::PasswordProtect => "AES encrypt document",
            QuickAction::RemovePassword => "Remove PDF security restriction",
            QuickAction::FlattenDocument => "Flatten annotations & forms",

            QuickAction::ExtractHere => "Unpack in current directory",
            QuickAction::ExtractToFolder => "Unpack into subfolder",
            QuickAction::ListContents => "View archive catalog",
            QuickAction::CompressFurther => "Recompress with maximum ratio",
            QuickAction::ConvertTo7z => "Convert archive to 7-Zip",
            QuickAction::ConvertToZip => "Convert archive to standard ZIP",
            QuickAction::TestIntegrity => "Check archive checksums",

            QuickAction::ConvertToGlb => "Package model as binary glTF",
            QuickAction::OptimizeGeometry => "Weld vertices & strip unused",
            QuickAction::Generate3dThumbnail => "Render camera snapshot",
            QuickAction::View3d => "Launch 3D preview window",
            QuickAction::DecimateMesh => "Reduce polygon triangle count",
            QuickAction::ExtractTextures => "Dump embedded image textures",
            QuickAction::ConvertToStl => "Export 3D printable mesh",

            QuickAction::CopyPath => "Copy full file path to clipboard",
            QuickAction::ShowInExplorer => "Select item in Explorer",
            QuickAction::CalculateHash => "Compute SHA-256 fingerprint",
        }
    }

    pub fn icon_symbol(&self) -> &'static str {
        match self {
            QuickAction::RemoveBackground => "✨",
            QuickAction::ResizeImage => "📐",
            QuickAction::CompressImage => "🗜️",
            QuickAction::UpscaleImage => "🔍",
            QuickAction::CropImage => "✂️",
            QuickAction::RotateImage => "🔄",
            QuickAction::RemoveMetadata => "🛡️",
            QuickAction::ConvertToIcon => "⭐",
            QuickAction::OptimizeImage => "⚡",
            QuickAction::BatchProcess => "📁",

            QuickAction::CompressVideo => "🗜️",
            QuickAction::ExtractAudio => "🎵",
            QuickAction::RemoveAudio => "🔇",
            QuickAction::TrimVideo => "✂️",
            QuickAction::ResizeVideo => "📐",
            QuickAction::ChangeFps => "⏱️",
            QuickAction::ExtractFrames => "🎞️",
            QuickAction::GenerateThumbnail => "🖼️",

            QuickAction::NormalizeVolume => "🔊",
            QuickAction::TrimAudio => "✂️",
            QuickAction::CompressAudio => "🗜️",
            QuickAction::ExtractAudioMetadata => "📋",
            QuickAction::RemoveAudioMetadata => "🛡️",
            QuickAction::ChangeSampleRate => "🎚️",
            QuickAction::ConvertToMono => "🎧",
            QuickAction::ConvertToStereo => "📻",

            QuickAction::ConvertToPdf => "📄",
            QuickAction::ExtractText => "📝",
            QuickAction::OcrDocument => "🔍",
            QuickAction::MergeDocuments => "🔗",
            QuickAction::SplitDocument => "✂️",
            QuickAction::CompressDocument => "🗜️",
            QuickAction::PasswordProtect => "🔒",
            QuickAction::RemovePassword => "🔓",
            QuickAction::FlattenDocument => "📑",

            QuickAction::ExtractHere => "📂",
            QuickAction::ExtractToFolder => "📁",
            QuickAction::ListContents => "📋",
            QuickAction::CompressFurther => "⚡",
            QuickAction::ConvertTo7z => "🗄️",
            QuickAction::ConvertToZip => "📦",
            QuickAction::TestIntegrity => "✅",

            QuickAction::ConvertToGlb => "🌐",
            QuickAction::OptimizeGeometry => "⚡",
            QuickAction::Generate3dThumbnail => "🖼️",
            QuickAction::View3d => "👁️",
            QuickAction::DecimateMesh => "📉",
            QuickAction::ExtractTextures => "🎨",
            QuickAction::ConvertToStl => "📐",

            QuickAction::CopyPath => "📋",
            QuickAction::ShowInExplorer => "📂",
            QuickAction::CalculateHash => "🔑",
        }
    }

    pub fn category_label(&self) -> &'static str {
        match self {
            QuickAction::RemoveBackground
            | QuickAction::ResizeImage
            | QuickAction::CompressImage
            | QuickAction::UpscaleImage
            | QuickAction::CropImage
            | QuickAction::RotateImage
            | QuickAction::RemoveMetadata
            | QuickAction::ConvertToIcon
            | QuickAction::OptimizeImage
            | QuickAction::BatchProcess => "IMG",

            QuickAction::CompressVideo
            | QuickAction::ExtractAudio
            | QuickAction::RemoveAudio
            | QuickAction::TrimVideo
            | QuickAction::ResizeVideo
            | QuickAction::ChangeFps
            | QuickAction::ExtractFrames
            | QuickAction::GenerateThumbnail => "VID",

            QuickAction::NormalizeVolume
            | QuickAction::TrimAudio
            | QuickAction::CompressAudio
            | QuickAction::ExtractAudioMetadata
            | QuickAction::RemoveAudioMetadata
            | QuickAction::ChangeSampleRate
            | QuickAction::ConvertToMono
            | QuickAction::ConvertToStereo => "AUD",

            QuickAction::ConvertToPdf
            | QuickAction::ExtractText
            | QuickAction::OcrDocument
            | QuickAction::MergeDocuments
            | QuickAction::SplitDocument
            | QuickAction::CompressDocument
            | QuickAction::PasswordProtect
            | QuickAction::RemovePassword
            | QuickAction::FlattenDocument => "DOC",

            QuickAction::ExtractHere
            | QuickAction::ExtractToFolder
            | QuickAction::ListContents
            | QuickAction::CompressFurther
            | QuickAction::ConvertTo7z
            | QuickAction::ConvertToZip
            | QuickAction::TestIntegrity => "ARCH",

            QuickAction::ConvertToGlb
            | QuickAction::OptimizeGeometry
            | QuickAction::Generate3dThumbnail
            | QuickAction::View3d
            | QuickAction::DecimateMesh
            | QuickAction::ExtractTextures
            | QuickAction::ConvertToStl => "3D",

            QuickAction::CopyPath | QuickAction::ShowInExplorer | QuickAction::CalculateHash => "FILE",
        }
    }

    pub fn badge_rgb(&self) -> (u8, u8, u8) {
        match self.category_label() {
            "IMG" => (168, 85, 247),   // Purple
            "VID" => (59, 130, 246),   // Blue
            "AUD" => (245, 158, 11),   // Amber
            "DOC" => (239, 68, 68),    // Red/Rose
            "ARCH" => (217, 119, 6),   // Orange
            "3D" => (16, 185, 129),    // Emerald
            _ => (100, 116, 139),      // Slate
        }
    }
}

/// Computes the contextual Quick Actions for a given file format.
/// Returns (inner_actions, outer_actions).
/// If total actions exceed 5, 5 are placed on the inner wheel and the rest on the outer wheel,
/// allowing the 6th sector on the inner wheel to act as "More Actions".
pub fn get_quick_actions_for_format(format: FileFormat) -> (Vec<QuickAction>, Vec<QuickAction>) {
    let all_actions: Vec<QuickAction> = if format.is_image() {
        vec![
            QuickAction::RemoveBackground,
            QuickAction::ResizeImage,
            QuickAction::CompressImage,
            QuickAction::RotateImage,
            QuickAction::RemoveMetadata,
            QuickAction::ConvertToIcon,
            QuickAction::CropImage,
            QuickAction::UpscaleImage,
            QuickAction::OptimizeImage,
            QuickAction::BatchProcess,
        ]
    } else if format.is_video() {
        vec![
            QuickAction::ExtractAudio,
            QuickAction::CompressVideo,
            QuickAction::RemoveAudio,
            QuickAction::GenerateThumbnail,
            QuickAction::ChangeFps,
            QuickAction::TrimVideo,
            QuickAction::ResizeVideo,
            QuickAction::ExtractFrames,
            QuickAction::RemoveMetadata,
        ]
    } else if format.is_audio() {
        vec![
            QuickAction::NormalizeVolume,
            QuickAction::CompressAudio,
            QuickAction::ConvertToMono,
            QuickAction::ConvertToStereo,
            QuickAction::ChangeSampleRate,
            QuickAction::TrimAudio,
            QuickAction::ExtractAudioMetadata,
            QuickAction::RemoveAudioMetadata,
        ]
    } else if format.is_document() || format.is_presentation() || format.is_spreadsheet() {
        vec![
            QuickAction::ConvertToPdf,
            QuickAction::ExtractText,
            QuickAction::CompressDocument,
            QuickAction::FlattenDocument,
            QuickAction::MergeDocuments,
            QuickAction::SplitDocument,
            QuickAction::OcrDocument,
            QuickAction::PasswordProtect,
            QuickAction::RemovePassword,
        ]
    } else if format.is_archive() {
        vec![
            QuickAction::ExtractHere,
            QuickAction::ExtractToFolder,
            QuickAction::ConvertToZip,
            QuickAction::ConvertTo7z,
            QuickAction::TestIntegrity,
            QuickAction::CompressFurther,
            QuickAction::ListContents,
        ]
    } else if format.is_3d() {
        vec![
            QuickAction::ConvertToGlb,
            QuickAction::ConvertToStl,
            QuickAction::OptimizeGeometry,
            QuickAction::Generate3dThumbnail,
            QuickAction::DecimateMesh,
            QuickAction::View3d,
            QuickAction::ExtractTextures,
        ]
    } else {
        vec![
            QuickAction::CopyPath,
            QuickAction::ShowInExplorer,
            QuickAction::CalculateHash,
        ]
    };

    if all_actions.len() <= 6 {
        (all_actions, Vec::new())
    } else {
        let inner = all_actions[..5].to_vec();
        let outer = all_actions[5..].to_vec();
        (inner, outer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_actions_image() {
        let (inner, outer) = get_quick_actions_for_format(FileFormat::Png);
        assert_eq!(inner.len(), 5);
        assert!(!outer.is_empty());
        assert_eq!(inner[0], QuickAction::RemoveBackground);
    }

    #[test]
    fn test_quick_actions_video() {
        let (inner, outer) = get_quick_actions_for_format(FileFormat::Mp4);
        assert_eq!(inner.len(), 5);
        assert!(!outer.is_empty());
        assert_eq!(inner[0], QuickAction::ExtractAudio);
    }

    #[test]
    fn test_quick_actions_audio() {
        let (inner, outer) = get_quick_actions_for_format(FileFormat::Mp3);
        assert_eq!(inner.len(), 5);
        assert!(!outer.is_empty());
        assert_eq!(inner[0], QuickAction::NormalizeVolume);
    }

    #[test]
    fn test_quick_actions_doc() {
        let (inner, outer) = get_quick_actions_for_format(FileFormat::Docx);
        assert_eq!(inner.len(), 5);
        assert!(!outer.is_empty());
        assert_eq!(inner[0], QuickAction::ConvertToPdf);
    }

    #[test]
    fn test_quick_actions_archive() {
        let (inner, outer) = get_quick_actions_for_format(FileFormat::Zip);
        assert_eq!(inner.len(), 5);
        assert!(!outer.is_empty());
        assert_eq!(inner[0], QuickAction::ExtractHere);
    }

    #[test]
    fn test_quick_actions_3d() {
        let (inner, outer) = get_quick_actions_for_format(FileFormat::Obj);
        assert_eq!(inner.len(), 5);
        assert!(!outer.is_empty());
        assert_eq!(inner[0], QuickAction::ConvertToGlb);
    }
}

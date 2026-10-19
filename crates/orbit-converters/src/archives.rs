use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct ArchiveConverter;

impl ArchiveConverter {
    pub fn new() -> Self {
        Self
    }
}

impl Converter for ArchiveConverter {
    fn id(&self) -> &'static str {
        "bsdtar-archive-engine"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        let is_archive_src = matches!(
            from,
            FileFormat::Zip
                | FileFormat::Tar
                | FileFormat::Gz
                | FileFormat::Bz2
                | FileFormat::Xz
                | FileFormat::Zst
                | FileFormat::SevenZip
        );

        let is_archive_dst = matches!(
            to,
            FileFormat::Zip
                | FileFormat::Tar
                | FileFormat::Gz
                | FileFormat::Bz2
                | FileFormat::Xz
                | FileFormat::Zst
        );

        is_archive_src && is_archive_dst && from != to
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        if let Some(cb) = progress {
            cb(0.1);
        }

        // Create temporary extraction directory
        let temp_dir = tempfile::Builder::new()
            .prefix("orbit_arch_")
            .tempdir()
            .map_err(|e| ConversionError::IoError(e.to_string()))?;

        // 1. Extract archive to temp directory using Windows native tar.exe
        let extract_status = Command::new("tar")
            .args(&["-xf"])
            .arg(input)
            .arg("-C")
            .arg(temp_dir.path())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| {
                ConversionError::ProcessingError(format!("Failed to execute Windows tar.exe: {}", e))
            })?;

        if !extract_status.success() {
            return Err(ConversionError::ProcessingError(
                "tar.exe extraction returned a non-zero exit status".to_string(),
            ));
        }

        if let Some(cb) = progress {
            cb(0.5);
        }

        // 2. Determine archive creation flag based on destination format
        let out_ext = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();

        let create_flag = match out_ext.as_str() {
            "zip" => "-acf",
            "tar" => "-cf",
            "gz" | "tgz" => "-czf",
            "bz2" | "tbz2" => "-cjf",
            "xz" | "txz" => "-cJf",
            "zst" => "-acf",
            _ => "-acf",
        };

        let create_status = Command::new("tar")
            .arg(create_flag)
            .arg(output)
            .arg(".")
            .current_dir(temp_dir.path())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| {
                ConversionError::ProcessingError(format!("Failed to pack target archive: {}", e))
            })?;

        if !create_status.success() {
            return Err(ConversionError::ProcessingError(
                "tar.exe creation returned a non-zero exit status".to_string(),
            ));
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        Ok(())
    }
}

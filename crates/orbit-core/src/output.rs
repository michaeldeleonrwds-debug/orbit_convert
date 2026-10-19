use crate::formats::FileFormat;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// RAII Guard for temporary output files.
/// Automatically deletes the temporary file if it still exists when dropped,
/// ensuring failed/aborted conversions leave no corrupt half-written files.
pub struct TempOutputGuard {
    temp_path: PathBuf,
    committed: bool,
}

impl TempOutputGuard {
    pub fn new(temp_path: PathBuf) -> Self {
        Self {
            temp_path,
            committed: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.temp_path
    }

    /// Mark the temporary file as committed (successfully converted and renamed),
    /// so the drop handler will not attempt to delete it.
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for TempOutputGuard {
    fn drop(&mut self) {
        if !self.committed && self.temp_path.exists() {
            let _ = fs::remove_file(&self.temp_path);
        }
    }
}

/// Resolves the final destination path and creates a managed temporary file guard.
/// Ensures collision safety (`name (1).ext`) and prevents overwriting the source file.
pub fn prepare_output_file(
    source_path: &Path,
    target_format: FileFormat,
    custom_destination_dir: Option<&Path>,
) -> Result<(PathBuf, TempOutputGuard), std::io::Error> {
    let parent_dir = match custom_destination_dir {
        Some(dir) => dir.to_path_buf(),
        None => source_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let file_stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("converted");

    let ext = target_format.primary_extension();

    // 1. Determine collision-safe target path
    let mut candidate_path = parent_dir.join(format!("{}.{}", file_stem, ext));

    // Never overwrite source file, and don't overwrite existing files
    let is_source = |path: &Path| -> bool {
        if let (Ok(can_can), Ok(src_can)) = (path.canonicalize(), source_path.canonicalize()) {
            can_can == src_can
        } else {
            path == source_path
        }
    };

    if candidate_path.exists() || is_source(&candidate_path) {
        let mut index = 1;
        loop {
            let next_name = format!("{} ({}).{}", file_stem, index, ext);
            let next_path = parent_dir.join(next_name);
            if !next_path.exists() && !is_source(&next_path) {
                candidate_path = next_path;
                break;
            }
            index += 1;
        }
    }

    // 2. Create unique temporary file path in the same target folder
    // (Placing it on the same filesystem ensures atomic rename)
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(
        ".orbit_{}_{}_{}.{}",
        file_stem, timestamp, counter, ext
    );
    let temp_path = parent_dir.join(temp_name);

    Ok((candidate_path, TempOutputGuard::new(temp_path)))
}

/// Safely finalize the temporary conversion file into its target path.
pub fn finalize_output(temp_guard: TempOutputGuard, target_path: &Path) -> Result<(), std::io::Error> {
    let temp_path = temp_guard.path().to_path_buf();

    // Ensure parent directories exist
    if let Some(parent) = target_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    // Atomic move on the same filesystem
    fs::rename(&temp_path, target_path)?;

    // Commit guard so it won't delete the file on drop
    temp_guard.commit();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_collision_safe_naming() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("photo.jpg");
        fs::write(&src_file, b"sample").unwrap();

        // Target is PNG. photo.png doesn't exist yet.
        let (out1, guard1) = prepare_output_file(&src_file, FileFormat::Png, None).unwrap();
        assert_eq!(out1, dir.path().join("photo.png"));
        fs::write(guard1.path(), b"data1").unwrap();
        finalize_output(guard1, &out1).unwrap();
        assert!(out1.exists());

        // Now photo.png exists. Second conversion should name photo (1).png
        let (out2, guard2) = prepare_output_file(&src_file, FileFormat::Png, None).unwrap();
        assert_eq!(out2, dir.path().join("photo (1).png"));
        fs::write(guard2.path(), b"data2").unwrap();
        finalize_output(guard2, &out2).unwrap();
        assert!(out2.exists());

        // Third should name photo (2).png
        let (out3, guard3) = prepare_output_file(&src_file, FileFormat::Png, None).unwrap();
        assert_eq!(out3, dir.path().join("photo (2).png"));
        guard3.commit(); // cancel/drop without finalize
    }

    #[test]
    fn test_never_overwrite_source() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("document.png");
        fs::write(&src_file, b"content").unwrap();

        // Trying to convert document.png to PNG must not overwrite source
        let (out, guard) = prepare_output_file(&src_file, FileFormat::Png, None).unwrap();
        assert_eq!(out, dir.path().join("document (1).png"));
        guard.commit();
    }

    #[test]
    fn test_temp_file_cleanup_on_drop() {
        let dir = tempdir().unwrap();
        let src_file = dir.path().join("image.svg");
        fs::write(&src_file, b"svg").unwrap();

        let temp_path;
        {
            let (_out, guard) = prepare_output_file(&src_file, FileFormat::Png, None).unwrap();
            temp_path = guard.path().to_path_buf();
            fs::write(&temp_path, b"partial corrupt output").unwrap();
            assert!(temp_path.exists());
            // guard dropped here without commit
        }

        // Temp file should be cleaned up automatically
        assert!(!temp_path.exists());
    }
}

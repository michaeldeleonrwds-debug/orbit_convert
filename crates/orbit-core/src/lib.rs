pub mod config;
pub mod formats;
pub mod jobs;
pub mod output;
pub mod registry;

pub use config::{AppConfig, ConversionPreset, WatchedFolder};
pub use formats::FileFormat;
pub use jobs::{JobId, JobManager, JobRecord, JobState};
pub use output::{finalize_output, prepare_output_file, TempOutputGuard};
pub use registry::{ConversionError, ConversionOptions, ConversionRegistry, Converter};

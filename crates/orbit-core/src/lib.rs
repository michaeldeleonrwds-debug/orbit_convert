pub mod actions;
pub mod config;
pub mod formats;
pub mod jobs;
pub mod output;
pub mod probe;
pub mod registry;

pub use actions::{get_quick_actions_for_format, QuickAction};
pub use config::{AppConfig, ConversionPreset, WatchedFolder};
pub use formats::FileFormat;
pub use jobs::{JobId, JobManager, JobRecord, JobState};
pub use output::{finalize_output, prepare_output_file, TempOutputGuard};
pub use probe::{probe_media, MediaStreamInfo};
pub use registry::{ConversionError, ConversionOptions, ConversionRegistry, Converter};

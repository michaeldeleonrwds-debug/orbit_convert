use crate::formats::FileFormat;
use crate::output::{finalize_output, prepare_output_file};
use crate::registry::{ConversionOptions, ConversionRegistry};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tracing::{error, info};

pub type JobId = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobState {
    Pending,
    Processing { progress: f32 },
    Completed {
        output_path: PathBuf,
        duration_ms: u64,
        output_size_bytes: u64,
    },
    Failed { error: String },
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct JobRecord {
    pub id: JobId,
    pub source_path: PathBuf,
    pub source_format: FileFormat,
    pub target_format: FileFormat,
    pub state: JobState,
    pub started_at: Option<Instant>,
}

pub type JobNotificationCallback = Arc<dyn Fn(&JobRecord) + Send + Sync>;

pub struct JobManager {
    registry: ConversionRegistry,
    options: Mutex<ConversionOptions>,
    next_id: std::sync::atomic::AtomicU64,
    history: Arc<Mutex<Vec<JobRecord>>>,
    sender: crossbeam_channel::Sender<ConversionTask>,
    notification_callback: Mutex<Option<JobNotificationCallback>>,
    active_cancellations: Arc<Mutex<std::collections::HashMap<JobId, Arc<AtomicBool>>>>,
    custom_dir_cache: Arc<Mutex<std::collections::HashMap<JobId, Option<PathBuf>>>>,
}

struct ConversionTask {
    id: JobId,
    source_path: PathBuf,
    source_format: FileFormat,
    target_format: FileFormat,
    options: ConversionOptions,
    custom_output_dir: Option<PathBuf>,
    cancel_flag: Arc<AtomicBool>,
}

impl JobManager {
    pub fn new(registry: ConversionRegistry, options: ConversionOptions) -> Arc<Self> {
        let (tx, rx) = crossbeam_channel::unbounded::<ConversionTask>();
        let history = Arc::new(Mutex::new(Vec::new()));
        let active_cancellations = Arc::new(Mutex::new(std::collections::HashMap::new()));
        let custom_dir_cache = Arc::new(Mutex::new(std::collections::HashMap::new()));

        let manager = Arc::new(Self {
            registry: registry.clone(),
            options: Mutex::new(options),
            next_id: std::sync::atomic::AtomicU64::new(1),
            history: history.clone(),
            sender: tx,
            notification_callback: Mutex::new(None),
            active_cancellations: active_cancellations.clone(),
            custom_dir_cache: custom_dir_cache.clone(),
        });

        // Spawn background worker thread
        let worker_manager = Arc::downgrade(&manager);
        thread::Builder::new()
            .name("orbit-conversion-worker".into())
            .spawn(move || {
                while let Ok(task) = rx.recv() {
                    if let Some(mgr) = worker_manager.upgrade() {
                        mgr.execute_task(task);
                    } else {
                        break;
                    }
                }
            })
            .expect("Failed to spawn conversion worker thread");

        manager
    }

    pub fn set_notification_callback(&self, callback: JobNotificationCallback) {
        let mut guard = self.notification_callback.lock().unwrap();
        *guard = Some(callback);
    }

    pub fn update_options(&self, options: ConversionOptions) {
        let mut guard = self.options.lock().unwrap();
        *guard = options;
    }

    pub fn get_options(&self) -> ConversionOptions {
        self.options.lock().unwrap().clone()
    }

    pub fn get_history(&self) -> Vec<JobRecord> {
        self.history.lock().unwrap().clone()
    }

    /// Cancel an active or pending conversion job
    pub fn cancel(&self, id: JobId) -> bool {
        let guard = self.active_cancellations.lock().unwrap();
        if let Some(flag) = guard.get(&id) {
            flag.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Cancel all currently active conversion jobs
    pub fn cancel_all(&self) {
        let guard = self.active_cancellations.lock().unwrap();
        for flag in guard.values() {
            flag.store(true, Ordering::Relaxed);
        }
    }

    /// Retry a previously failed conversion job
    pub fn retry(&self, id: JobId) -> Result<JobId, String> {
        let (src_path, src_fmt, target_fmt, custom_out) = {
            let hist = self.history.lock().unwrap();
            let rec = hist.iter().find(|r| r.id == id).ok_or_else(|| {
                format!("Job #{} not found in history", id)
            })?;
            let custom_dir = self.custom_dir_cache.lock().unwrap().get(&id).cloned().flatten();
            (rec.source_path.clone(), rec.source_format, rec.target_format, custom_dir)
        };

        self.submit(src_path, src_fmt, target_fmt, custom_out)
    }

    /// Submit a conversion request to the background queue.
    pub fn submit(
        &self,
        source_path: PathBuf,
        source_format: FileFormat,
        target_format: FileFormat,
        custom_output_dir: Option<PathBuf>,
    ) -> Result<JobId, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut options = self.get_options();
        options.cancel_flag = Some(cancel_flag.clone());

        {
            let mut c_guard = self.active_cancellations.lock().unwrap();
            c_guard.insert(id, cancel_flag.clone());
            let mut dir_guard = self.custom_dir_cache.lock().unwrap();
            dir_guard.insert(id, custom_output_dir.clone());
        }

        let record = JobRecord {
            id,
            source_path: source_path.clone(),
            source_format,
            target_format,
            state: JobState::Pending,
            started_at: None,
        };

        {
            let mut hist = self.history.lock().unwrap();
            hist.push(record.clone());
        }

        self.notify(&record);

        let task = ConversionTask {
            id,
            source_path,
            source_format,
            target_format,
            options,
            custom_output_dir,
            cancel_flag,
        };

        self.sender
            .send(task)
            .map_err(|e| format!("Failed to enqueue conversion job: {}", e))?;

        Ok(id)
    }

    fn notify(&self, record: &JobRecord) {
        if let Some(cb) = self.notification_callback.lock().unwrap().as_ref() {
            cb(record);
        }
    }

    fn update_job_state<F>(&self, id: JobId, updater: F) -> Option<JobRecord>
    where
        F: FnOnce(&mut JobRecord),
    {
        let mut hist = self.history.lock().unwrap();
        if let Some(rec) = hist.iter_mut().find(|r| r.id == id) {
            updater(rec);
            let updated = rec.clone();
            drop(hist);
            self.notify(&updated);
            Some(updated)
        } else {
            None
        }
    }

    fn execute_task(&self, task: ConversionTask) {
        let start_time = Instant::now();

        self.update_job_state(task.id, |rec| {
            rec.started_at = Some(start_time);
            rec.state = JobState::Processing { progress: 0.0 };
        });

        info!(
            job_id = task.id,
            input = %task.source_path.display(),
            target = %task.target_format,
            "Starting conversion"
        );

        let converter = match self.registry.find_converter(task.source_format, task.target_format) {
            Some(c) => c,
            None => {
                let err_msg = format!(
                    "No converter registered for {} -> {}",
                    task.source_format, task.target_format
                );
                error!(job_id = task.id, "{err_msg}");
                self.update_job_state(task.id, |rec| {
                    rec.state = JobState::Failed { error: err_msg };
                });
                return;
            }
        };

        if task.cancel_flag.load(Ordering::Relaxed) {
            self.update_job_state(task.id, |rec| {
                rec.state = JobState::Cancelled;
            });
            return;
        }

        let (target_path, temp_guard) = match prepare_output_file(
            &task.source_path,
            task.target_format,
            task.custom_output_dir.as_deref(),
        ) {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("Failed to prepare output destination: {}", e);
                error!(job_id = task.id, "{err_msg}");
                self.update_job_state(task.id, |rec| {
                    rec.state = JobState::Failed { error: err_msg };
                });
                return;
            }
        };

        let temp_path = temp_guard.path().to_path_buf();
        let cancel_flag = task.cancel_flag.clone();

        let last_reported_pct = std::cell::Cell::new(-1i32);
        let progress_cb = |p: f32| {
            let pct = (p * 100.0).round() as i32;
            if pct != last_reported_pct.get() {
                last_reported_pct.set(pct);
                self.update_job_state(task.id, |rec| {
                    rec.state = JobState::Processing { progress: p };
                });
            }
        };

        let convert_result = converter.convert(
            &task.source_path,
            &temp_path,
            &task.options,
            Some(&progress_cb),
        );

        if cancel_flag.load(Ordering::Relaxed) {
            drop(temp_guard); // Cleans up temp file
            self.update_job_state(task.id, |rec| {
                rec.state = JobState::Cancelled;
            });
            return;
        }

        match convert_result {
            Ok(()) => {
                if let Err(e) = finalize_output(temp_guard, &target_path) {
                    let err_msg = format!("Failed to move converted file into place: {}", e);
                    error!(job_id = task.id, "{err_msg}");
                    self.update_job_state(task.id, |rec| {
                        rec.state = JobState::Failed { error: err_msg };
                    });
                } else {
                    let elapsed = start_time.elapsed().as_millis() as u64;
                    info!(
                        job_id = task.id,
                        output = %target_path.display(),
                        duration_ms = elapsed,
                        "Conversion completed successfully"
                    );
                    let size_bytes = std::fs::metadata(&target_path)
                        .map(|m| m.len())
                        .unwrap_or(0);
                    self.update_job_state(task.id, |rec| {
                        rec.state = JobState::Completed {
                            output_path: target_path,
                            duration_ms: elapsed,
                            output_size_bytes: size_bytes,
                        };
                    });
                }
            }
            Err(e) => {
                drop(temp_guard); // Cleans up temp file
                let err_msg = e.to_string();
                error!(job_id = task.id, "Conversion failed: {err_msg}");
                self.update_job_state(task.id, |rec| {
                    rec.state = JobState::Failed { error: err_msg };
                });
            }
        }

        self.active_cancellations.lock().unwrap().remove(&task.id);
    }
}

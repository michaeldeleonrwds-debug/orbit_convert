use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command: String,
    pub job_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Value>,
    #[serde(default = "default_version")]
    pub protocol_version: String,
}

fn default_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub job_id: String,
    pub operation: String,
    pub status: String, // "started", "progress", "success", "error", "cancelled"
    #[serde(default)]
    pub input_file: Option<String>,
    #[serde(default)]
    pub output_file: Option<String>,
    #[serde(default)]
    pub progress: Option<f32>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub processing_time_ms: Option<f64>,
    #[serde(default)]
    pub payload: Option<Value>,
}

struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

pub struct PythonWorkerManager {
    worker: Mutex<Option<WorkerProcess>>,
    python_path: Mutex<Option<PathBuf>>,
    python_root: Mutex<Option<PathBuf>>,
}

impl PythonWorkerManager {
    pub fn new() -> Self {
        Self {
            worker: Mutex::new(None),
            python_path: Mutex::new(None),
            python_root: Mutex::new(None),
        }
    }

    /// Finds a valid Python executable
    pub fn find_python_exe(&self) -> Option<PathBuf> {
        let mut cached = self.python_path.lock().unwrap();
        if let Some(ref p) = *cached {
            if p.exists() {
                return Some(p.clone());
            }
        }

        // 1. Environment variable
        if let Ok(env_path) = std::env::var("ORBIT_PYTHON_PATH") {
            let p = PathBuf::from(env_path);
            if p.exists() {
                *cached = Some(p.clone());
                return Some(p);
            }
        }

        // 2. Relative to current exe (for packaged app)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(dir) = exe_path.parent() {
                let candidates = [
                    dir.join("python").join("python.exe"),
                    dir.join("python_runtime").join("python.exe"),
                    dir.join("..").join("python").join("python.exe"),
                ];
                for cand in &candidates {
                    if cand.exists() {
                        *cached = Some(cand.clone());
                        return Some(cand.clone());
                    }
                }
            }
        }

        // 3. User local Python (Windows AppData)
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let programs = Path::new(&local_app_data).join("Programs").join("Python");
            let candidates = [
                programs.join("Python311").join("python.exe"),
                programs.join("Python312").join("python.exe"),
                programs.join("Python310").join("python.exe"),
            ];
            for cand in &candidates {
                if cand.exists() {
                    *cached = Some(cand.clone());
                    return Some(cand.clone());
                }
            }
        }

        // 4. System PATH search
        if let Ok(path_var) = std::env::var("PATH") {
            for part in std::env::split_paths(&path_var) {
                for name in &["python.exe", "python3.exe", "python", "python3"] {
                    let cand = part.join(name);
                    if cand.is_file() {
                        *cached = Some(cand.clone());
                        return Some(cand);
                    }
                }
            }
        }

        None
    }

    /// Finds the python/ directory containing the orbit_engine package
    pub fn find_python_root(&self) -> Option<PathBuf> {
        let mut cached = self.python_root.lock().unwrap();
        if let Some(ref p) = *cached {
            if p.exists() && p.join("orbit_engine").exists() {
                return Some(p.clone());
            }
        }

        // 1. Environment variable
        if let Ok(env_root) = std::env::var("ORBIT_PYTHON_ROOT") {
            let p = PathBuf::from(env_root);
            if p.exists() && p.join("orbit_engine").exists() {
                *cached = Some(p.clone());
                return Some(p);
            }
        }

        // 2. Relative to current working dir and its ancestors (supports running inside subcrates)
        if let Ok(cwd) = std::env::current_dir() {
            let mut curr = Some(cwd.as_path());
            while let Some(dir) = curr {
                let cand = dir.join("python");
                if cand.exists() && cand.join("orbit_engine").exists() {
                    let full = std::fs::canonicalize(&cand).unwrap_or(cand);
                    *cached = Some(full.clone());
                    return Some(full);
                }
                curr = dir.parent();
            }
        }

        // 3. Relative to CARGO_MANIFEST_DIR (for cargo test)
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let mut curr = Some(Path::new(&manifest_dir));
            while let Some(dir) = curr {
                let cand = dir.join("python");
                if cand.exists() && cand.join("orbit_engine").exists() {
                    let full = std::fs::canonicalize(&cand).unwrap_or(cand);
                    *cached = Some(full.clone());
                    return Some(full);
                }
                curr = dir.parent();
            }
        }

        // 4. Relative to current exe and its ancestors (for release / debug binaries)
        if let Ok(exe) = std::env::current_exe() {
            let mut curr = exe.parent();
            while let Some(dir) = curr {
                let cand = dir.join("python");
                if cand.exists() && cand.join("orbit_engine").exists() {
                    let full = std::fs::canonicalize(&cand).unwrap_or(cand);
                    *cached = Some(full.clone());
                    return Some(full);
                }
                curr = dir.parent();
            }
        }

        None
    }

    pub fn is_available(&self) -> bool {
        self.find_python_exe().is_some() && self.find_python_root().is_some()
    }

    fn ensure_worker_started(&self) -> Result<(), String> {
        let mut lock = self.worker.lock().unwrap();
        if let Some(ref mut proc) = *lock {
            // Check if process is still running
            match proc.child.try_wait() {
                Ok(None) => return Ok(()), // Still running
                _ => {
                    warn!("Python worker process exited unexpectedly. Restarting...");
                    *lock = None;
                }
            }
        }

        let python_exe = self
            .find_python_exe()
            .ok_or_else(|| "Python executable not found on system".to_string())?;
        let python_root = self
            .find_python_root()
            .ok_or_else(|| "Python orbit_engine root directory not found".to_string())?;

        info!(
            exe = %python_exe.display(),
            root = %python_root.display(),
            "Starting persistent OrbitConvert Python Worker daemon"
        );

        let mut cmd = Command::new(&python_exe);
        cmd.arg("-m")
            .arg("orbit_engine.worker")
            .current_dir(&python_root)
            .env("PYTHONPATH", &python_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn Python worker: {}", e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture Python worker stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture Python worker stdout".to_string())?;

        // Pipe stderr to tracing in a background thread to prevent deadlocks
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(l) = line {
                        debug!(target: "orbit_engine_python", "{}", l);
                    }
                }
            });
        }

        *lock = Some(WorkerProcess {
            child,
            stdin,
            reader: BufReader::new(stdout),
        });

        Ok(())
    }

    pub fn send_command(
        &self,
        request: CommandRequest,
        progress_cb: Option<Arc<dyn Fn(f32, &str) + Send + Sync>>,
    ) -> Result<CommandResponse, String> {
        let target_job_id = request.job_id.clone();
        let json_line = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        // Ensure process is running
        self.ensure_worker_started()?;

        let mut lock = self.worker.lock().unwrap();
        let proc = lock
            .as_mut()
            .ok_or_else(|| "Worker process unavailable".to_string())?;

        // Write line to stdin
        proc.stdin
            .write_all(json_line.as_bytes())
            .and_then(|_| proc.stdin.write_all(b"\n"))
            .and_then(|_| proc.stdin.flush())
            .map_err(|e| format!("Failed to write command to Python worker: {}", e))?;

        // Read responses until terminal status (success, error, cancelled)
        let timeout = Duration::from_secs(120);
        let start = Instant::now();

        let mut line_buffer = String::new();
        loop {
            if start.elapsed() > timeout {
                return Err("Python worker operation timed out".to_string());
            }

            line_buffer.clear();
            let bytes_read = proc
                .reader
                .read_line(&mut line_buffer)
                .map_err(|e| format!("Failed reading from Python worker: {}", e))?;

            if bytes_read == 0 {
                return Err("Python worker closed stdout unexpectedly".to_string());
            }

            let trimmed = line_buffer.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<CommandResponse>(trimmed) {
                Ok(resp) => {
                    if resp.job_id != target_job_id {
                        continue;
                    }

                    if resp.status == "progress" {
                        if let Some(ref cb) = progress_cb {
                            let p = resp.progress.unwrap_or(0.0);
                            let stage = resp.stage.as_deref().unwrap_or("Processing");
                            cb(p, stage);
                        }
                        continue;
                    }

                    if resp.status == "success" {
                        return Ok(resp);
                    } else if resp.status == "cancelled" {
                        return Err("Operation was cancelled".to_string());
                    } else {
                        let err_msg = resp
                            .error_message
                            .unwrap_or_else(|| "Unknown worker error".to_string());
                        return Err(err_msg);
                    }
                }
                Err(e) => {
                    debug!("Ignoring non-protocol output line: {} (err: {})", trimmed, e);
                }
            }
        }
    }

    /// Convenience: query engine capabilities
    pub fn get_capabilities(&self) -> Result<Value, String> {
        let req = CommandRequest {
            command: "capabilities".to_string(),
            job_id: format!("cap_{}", uuid_short()),
            input_file: None,
            output_file: None,
            action: None,
            options: None,
            protocol_version: "1.0".to_string(),
        };
        let resp = self.send_command(req, None)?;
        resp.payload
            .ok_or_else(|| "No payload in capabilities response".to_string())
    }

    /// Convenience: query dependency status
    pub fn get_dependency_status(&self) -> Result<Value, String> {
        let req = CommandRequest {
            command: "dependency_status".to_string(),
            job_id: format!("dep_{}", uuid_short()),
            input_file: None,
            output_file: None,
            action: None,
            options: None,
            protocol_version: "1.0".to_string(),
        };
        let resp = self.send_command(req, None)?;
        resp.payload
            .ok_or_else(|| "No payload in dependency response".to_string())
    }

    /// Convenience: execute AI background removal
    pub fn remove_background(
        &self,
        input_path: &Path,
        output_path: &Path,
        model: Option<&str>,
        alpha_matting: bool,
        progress_cb: Option<Arc<dyn Fn(f32, &str) + Send + Sync>>,
    ) -> Result<PathBuf, String> {
        let mut opts = serde_json::json!({
            "alpha_matting": alpha_matting,
        });
        if let Some(m) = model {
            opts["model"] = Value::String(m.to_string());
        }

        let req = CommandRequest {
            command: "quick_action".to_string(),
            job_id: format!("nobg_{}", uuid_short()),
            input_file: Some(input_path.to_string_lossy().to_string()),
            output_file: Some(output_path.to_string_lossy().to_string()),
            action: Some("remove_background".to_string()),
            options: Some(opts),
            protocol_version: "1.0".to_string(),
        };

        let resp = self.send_command(req, progress_cb)?;
        if let Some(out) = resp.output_file {
            Ok(PathBuf::from(out))
        } else {
            Ok(output_path.to_path_buf())
        }
    }
}

fn uuid_short() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// Global lazy singleton
static WORKER_MANAGER: once_cell::sync::Lazy<Arc<PythonWorkerManager>> =
    once_cell::sync::Lazy::new(|| Arc::new(PythonWorkerManager::new()));

pub fn get_python_worker() -> Arc<PythonWorkerManager> {
    WORKER_MANAGER.clone()
}

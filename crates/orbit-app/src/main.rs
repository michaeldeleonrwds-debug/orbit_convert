#![windows_subsystem = "windows"]

mod quick_actions_exec;
mod tray;

use orbit_converters::register_all_converters;
use orbit_core::actions::{get_quick_actions_for_format, QuickAction};
use orbit_core::config::{AppConfig, ConversionPreset, WatchedFolder};
use orbit_core::formats::FileFormat;
use orbit_core::jobs::{JobManager, JobRecord, JobState};
use orbit_core::registry::{ConversionOptions, ConversionRegistry};
use orbit_ui::{
    build_two_level_action_sectors, build_two_level_wheel_sectors, FolderItem,
    FormatCapabilityItem, HistoryItem, MainWindow, PresetItem, RadialWheel,
};
use orbit_windows::cursor::{
    calculate_clamped_window_pos_physical, calculate_two_level_selection_normalized,
    get_current_cursor_pos, get_monitor_scale_factor, is_more_sector_active,
    normalize_cursor_to_logical, set_current_cursor_pos, Point, TwoLevelSelection,
};
use orbit_windows::dialogs::{pick_file, pick_folder};
use orbit_windows::explorer::{ExplorerDiagnosticReport, ExplorerResolver};
use orbit_windows::keyboard::{
    is_cursor_or_foreground_explorer, KeyboardHookManager, RawKeyEvent,
};
use slint::{ComponentHandle, LogicalPosition, ModelRc, PhysicalPosition, VecModel};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Idle,
    WheelActive,
    Converting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelKind {
    Convert,
    QuickActions,
}

// Wheel interaction state supporting two-level concentric radial menu (Conversion & Quick Actions)
struct InteractionData {
    state: AppState,
    wheel_kind: WheelKind,
    center_pos: Point,
    scale_factor: f32,
    source_path: Option<PathBuf>,
    source_format: Option<FileFormat>,
    inner_targets: Vec<FileFormat>,
    outer_targets: Vec<FileFormat>,
    inner_actions: Vec<QuickAction>,
    outer_actions: Vec<QuickAction>,
    has_more: bool,
    is_expanded: bool,
    current_highlighted_inner: i32,
    current_highlighted_outer: i32,
    is_preview: bool,
}

fn make_folders_model(folders: &[WatchedFolder]) -> ModelRc<FolderItem> {
    let items: Vec<FolderItem> = folders
        .iter()
        .map(|f| FolderItem {
            id: f.id.clone().into(),
            name: f.name.clone().into(),
            path: f.path.clone().into(),
            enabled: f.enabled,
            is_watched: f.is_watched,
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

fn make_presets_model(presets: &[ConversionPreset]) -> ModelRc<PresetItem> {
    let items: Vec<PresetItem> = presets
        .iter()
        .map(|p| PresetItem {
            id: p.id.clone().into(),
            name: p.name.clone().into(),
            desc: p.description.clone().into(),
            target_format: p.target_format.clone().into(),
            quality: p.quality as i32,
            svg_scale: p.svg_scale,
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

fn make_capability_matrix_model(
    matrix: &[orbit_core::registry::CapabilityInfo],
    filter_query: &str,
) -> ModelRc<FormatCapabilityItem> {
    let clean_query = filter_query.trim().to_lowercase();
    let items: Vec<FormatCapabilityItem> = matrix
        .iter()
        .filter(|cap| {
            if clean_query.is_empty() {
                return true;
            }
            cap.format.primary_extension().to_lowercase().contains(&clean_query)
                || cap.format.display_label().to_lowercase().contains(&clean_query)
                || cap.format.category().to_string().to_lowercase().contains(&clean_query)
                || cap.engine.to_lowercase().contains(&clean_query)
        })
        .map(|cap| {
            let (status_label, status_color) = match cap.status {
                orbit_core::registry::DependencyStatus::Builtin => (
                    "Built-in (Native)",
                    slint::Color::from_rgb_u8(16, 185, 129),
                ),
                orbit_core::registry::DependencyStatus::Active => (
                    "Active Engine",
                    slint::Color::from_rgb_u8(56, 189, 248),
                ),
                orbit_core::registry::DependencyStatus::Missing => (
                    "Requires External Engine",
                    slint::Color::from_rgb_u8(245, 158, 11),
                ),
            };
            FormatCapabilityItem {
                name: cap.format.display_label().into(),
                extension: cap.format.primary_extension().into(),
                category: cap.format.category().to_string().into(),
                can_decode: cap.can_decode,
                can_encode: cap.can_encode,
                engine: cap.engine.into(),
                status_label: status_label.into(),
                status_color,
                tested_status: cap.tested_status.into(),
                limitations: cap.known_limitations.into(),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(items))
}

fn format_file_size(bytes: u64) -> String {
    if bytes == 0 {
        return String::new();
    }
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn main() {
    // Attach to parent console if run from CLI (PowerShell / cmd.exe)
    unsafe {
        let _ = windows::Win32::System::Console::AttachConsole(
            windows::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }

    // Initialize tracing logger
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting OrbitConvert v0.1.0...");

    let args: Vec<String> = env::args().collect();

    // Check CLI arguments
    if args.iter().any(|arg| arg == "--diagnose" || arg == "-d") {
        run_standalone_diagnostics(None);
        return;
    }

    if let Some(pos) = args.iter().position(|arg| arg == "--diagnose-point") {
        if pos + 2 < args.len() {
            if let (Ok(x), Ok(y)) = (args[pos + 1].parse::<f32>(), args[pos + 2].parse::<f32>()) {
                run_standalone_diagnostics(Some(Point::new(x, y)));
                return;
            }
        }
    }

    // Load configuration
    let config = Arc::new(Mutex::new(AppConfig::load()));

    // Setup conversion registry
    let mut registry = ConversionRegistry::new();
    register_all_converters(&mut registry);

    let initial_config = config.lock().unwrap().clone();
    let conversion_options = ConversionOptions {
        jpeg_quality: initial_config.jpeg_quality,
        webp_quality: initial_config.webp_quality,
        webp_lossless: false,
        svg_scale: initial_config.svg_scale,
        svg_target_width: None,
        svg_target_height: None,
        background_color: (
            initial_config.jpeg_bg_color[0],
            initial_config.jpeg_bg_color[1],
            initial_config.jpeg_bg_color[2],
        ),
        ico_sizes: vec![16, 32, 48, 64, 128, 256],
        ..ConversionOptions::default()
    };

    let job_manager = JobManager::new(registry.clone(), conversion_options);

    // Initialize UI: MainWindow Dashboard and RadialWheel Overlay
    let main_window = MainWindow::new().expect("Failed to initialize Slint MainWindow UI");
    let wheel = RadialWheel::new().expect("Failed to initialize Slint RadialWheel UI");

    // Initialize System Tray
    let mut tray = match tray::TrayManager::new() {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to create system tray icon: {}", e);
            return;
        }
    };
    tray.set_global();

    // In-memory History store
    let history_store: Arc<Mutex<Vec<HistoryItem>>> = Arc::new(Mutex::new(Vec::new()));
    let active_conversion_id: Arc<Mutex<Option<u64>>> = Arc::new(Mutex::new(None));

    // Populate initial MainWindow properties from AppConfig
    {
        main_window.set_jpeg_quality(initial_config.jpeg_quality as i32);
        main_window.set_svg_scale(initial_config.svg_scale);
        main_window.set_show_notifications(initial_config.show_notifications);
        main_window.set_launch_on_startup(initial_config.launch_on_startup);
        main_window.set_minimize_to_tray(initial_config.minimize_to_tray);
        main_window.set_service_active(true);
        main_window.set_hotkey_wheel_a(initial_config.hotkey_wheel_a.clone().into());
        main_window.set_hotkey_wheel_b(initial_config.hotkey_wheel_b.clone().into());
        main_window.set_hotkey_command_palette(initial_config.hotkey_command_palette.clone().into());
        main_window.set_show_onboarding(initial_config.show_onboarding);
        main_window.set_diag_timestamp("Ready".into());
        main_window.set_is_running_diagnostics(false);

        if let Some(ref custom_path) = initial_config.custom_output_dir {
            main_window.set_output_mode("custom".into());
            main_window.set_output_folder_label(custom_path.display().to_string().into());
        } else {
            main_window.set_output_mode("source".into());
            main_window.set_output_folder_label("Same as source file".into());
        }

        main_window.set_folders(make_folders_model(&initial_config.folders));
        main_window.set_presets(make_presets_model(&initial_config.presets));
        main_window.set_diag_ffmpeg_installed(
            orbit_converters::ffmpeg::FfmpegConverter::check_ffmpeg_available(),
        );

        let cap_matrix = registry.get_capability_matrix();
        main_window.set_capability_matrix(make_capability_matrix_model(&cap_matrix, ""));
    }

    let interaction = Arc::new(Mutex::new(InteractionData {
        state: AppState::Idle,
        wheel_kind: WheelKind::Convert,
        center_pos: Point::new(0.0, 0.0),
        scale_factor: 1.0,
        source_path: None,
        source_format: None,
        inner_targets: Vec::new(),
        outer_targets: Vec::new(),
        inner_actions: Vec::new(),
        outer_actions: Vec::new(),
        has_more: false,
        is_expanded: false,
        current_highlighted_inner: -1,
        current_highlighted_outer: -1,
        is_preview: false,
    }));

    // Set notification callback for completed/failed/in-progress jobs
    let main_window_for_notify = main_window.as_weak();
    let wheel_for_notify = wheel.as_weak();
    let history_store_for_notify = history_store.clone();
    let config_for_notify = config.clone();
    let interaction_for_notify = interaction.clone();

    // Track live progress notifications per job to throttle desktop popups smoothly
    let progress_notif_tracker = Arc::new(Mutex::new(std::collections::HashMap::<u64, (i32, Instant)>::new()));
    let tracker_clone = progress_notif_tracker.clone();

    job_manager.set_notification_callback(Arc::new(move |record: &JobRecord| {
        let show_notifs = config_for_notify.lock().unwrap().show_notifications;

        // If conversion is in-progress, update live progress readouts and send desktop progress notification
        if let JobState::Processing { progress } = record.state {
            let pct = (progress * 100.0).round() as i32;
            let status_msg = format!("Converting ... {}%", pct);

            let m_weak = main_window_for_notify.clone();
            let status_m = status_msg.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(m) = m_weak.upgrade() {
                    m.set_is_converting(true);
                    m.set_convert_progress(progress);
                    m.set_convert_status_msg(status_m.into());
                }
            });

            let w_weak = wheel_for_notify.clone();
            let status_w = status_msg.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = w_weak.upgrade() {
                    w.set_status_text(status_w.into());
                }
            });

            let fname = record
                .source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());

            // Always update tray icon tooltip with real-time conversion progress
            tray::update_global_tooltip(&format!(
                "OrbitConvert: {} -> {} ({}%)",
                fname,
                record.target_format.display_label(),
                pct
            ));

            // Show Windows notification with progress updates (throttled cleanly to avoid chime spam)
            if show_notifs {
                let mut tracker = tracker_clone.lock().unwrap();
                let should_notify = match tracker.get(&record.id) {
                    None => true, // First notification when starting
                    Some(&(last_pct, last_time)) => {
                        let elapsed_ms = last_time.elapsed().as_millis();
                        let delta = pct - last_pct;
                        // Trigger on key milestones (25%, 50%, 75%, 90%) or at least 1.5s + 15% progress
                        (last_pct < 25 && pct >= 25)
                            || (last_pct < 50 && pct >= 50)
                            || (last_pct < 75 && pct >= 75)
                            || (last_pct < 90 && pct >= 90)
                            || (elapsed_ms >= 1500 && delta >= 15)
                    }
                };

                if should_notify {
                    tracker.insert(record.id, (pct, Instant::now()));
                    let title = format!(
                        "OrbitConvert: Converting to {} ({}%)",
                        record.target_format.display_label(),
                        pct
                    );
                    let msg = format!("Converting {} ... {}%", fname, pct);
                    tray::send_global_progress_notification(&title, &msg);
                }
            }

            return;
        }

        // Clean up progress tracker and restore default tray tooltip
        tracker_clone.lock().unwrap().remove(&record.id);
        tray::update_global_tooltip("OrbitConvert - Hold Alt to Convert");

        let is_success = matches!(&record.state, JobState::Completed { .. });

        let (status_text, out_path_str, duration_ms, output_size_str) = match &record.state {
            JobState::Completed {
                output_path,
                duration_ms,
                output_size_bytes,
            } => {
                let fname = output_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let dur_secs = *duration_ms as f64 / 1000.0;
                let dur_str = if dur_secs < 0.1 {
                    format!("{:.2}s", dur_secs)
                } else if dur_secs < 10.0 {
                    format!("{:.1}s", dur_secs)
                } else {
                    format!("{:.1}s", dur_secs)
                };
                let size_str = format_file_size(*output_size_bytes);

                if show_notifs {
                    let title = format!("OrbitConvert: {} Ready", record.target_format);
                    let msg = if size_str.is_empty() {
                        format!("Saved {} in {}", fname, dur_str)
                    } else {
                        format!("Saved {} ({}) in {}", fname, size_str, dur_str)
                    };
                    tray::send_global_notification(&title, &msg, false);
                }
                (
                    format!("Completed in {}", dur_str),
                    output_path.display().to_string(),
                    *duration_ms as i32,
                    size_str,
                )
            }
            JobState::Failed { error } => {
                if show_notifs {
                    tray::send_global_notification("OrbitConvert: Conversion Failed", error, true);
                }
                (format!("Failed: {}", error), String::new(), 0, String::new())
            }
            _ => return,
        };

        let hist_item = HistoryItem {
            id: record.id as i32,
            source_name: record
                .source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
                .into(),
            source_ext: record.source_format.display_label().into(),
            target_ext: record.target_format.display_label().into(),
            output_path: out_path_str.clone().into(),
            duration_ms,
            output_size_str: output_size_str.into(),
            is_success,
            status_text: status_text.clone().into(),
        };

        {
            let mut hist = history_store_for_notify.lock().unwrap();
            hist.insert(0, hist_item);
            if hist.len() > 100 {
                hist.truncate(100);
            }
        }

        let m_weak = main_window_for_notify.clone();
        let hist = history_store_for_notify.lock().unwrap().clone();
        let status_for_ui = status_text.clone();
        let out_for_ui = out_path_str.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(m) = m_weak.upgrade() {
                m.set_history(ModelRc::new(VecModel::from(hist)));
                m.set_is_converting(false);
                m.set_convert_progress(1.0);
                m.set_convert_last_output(out_for_ui.into());
                m.set_convert_status_msg(if is_success {
                    "Conversion completed successfully!".into()
                } else {
                    format!("Error: {}", status_for_ui).into()
                });
            }
        });

        let w_weak = wheel_for_notify.clone();
        let target_lbl = record.target_format.display_label().to_string();
        let interaction_for_done = interaction_for_notify.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = w_weak.upgrade() {
                if is_success {
                    w.set_status_text(format!("✓ Converted to {}!", target_lbl).into());
                } else {
                    w.set_status_text("Conversion failed".into());
                }
            }

            let w_hide = w_weak.clone();
            slint::Timer::single_shot(Duration::from_millis(800), move || {
                if let Some(w) = w_hide.upgrade() {
                    let _ = w.hide();
                }
                let mut data = interaction_for_done.lock().unwrap();
                data.state = AppState::Idle;
                data.current_highlighted_inner = -1;
                data.current_highlighted_outer = -1;
                data.is_expanded = false;
            });
        });
    }));

    // Install global low-level keyboard hook
    let (_hook_mgr, key_rx) = match KeyboardHookManager::install() {
        Ok(res) => res,
        Err(e) => {
            error!("Failed to install keyboard hook: {}", e);
            return;
        }
    };

    orbit_windows::configure_hotkeys(orbit_windows::HotkeySettings {
        wheel_a: initial_config.hotkey_wheel_a.clone(),
        wheel_b: initial_config.hotkey_wheel_b.clone(),
        command_palette: initial_config.hotkey_command_palette.clone(),
    });

    let resolver = Arc::new(ExplorerResolver::new());

    // -------------------------------------------------------------
    // MainWindow Callbacks
    // -------------------------------------------------------------
    {
        let m_weak = main_window.as_weak();
        main_window.on_nav_clicked(move |idx| {
            if let Some(m) = m_weak.upgrade() {
                m.set_selected_nav(idx);
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let is_enabled = tray.is_enabled.clone();
        main_window.on_toggle_service_clicked(move || {
            let cur = is_enabled.load(Ordering::Relaxed);
            is_enabled.store(!cur, Ordering::Relaxed);
            if let Some(m) = m_weak.upgrade() {
                m.set_service_active(!cur);
            }
            info!("OrbitConvert service active: {}", !cur);
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_add_folder_clicked(move || {
            if let Some(folder_path) = pick_folder(None) {
                let name = folder_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Folder".to_string());
                let id = format!(
                    "f-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                );
                let mut cfg = config_clone.lock().unwrap();
                cfg.folders.push(WatchedFolder {
                    id,
                    name,
                    path: folder_path.display().to_string(),
                    enabled: true,
                    is_watched: false,
                });
                let _ = cfg.save();
                if let Some(m) = m_weak.upgrade() {
                    m.set_folders(make_folders_model(&cfg.folders));
                }
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_toggle_folder(move |id, enabled| {
            let mut cfg = config_clone.lock().unwrap();
            if let Some(f) = cfg.folders.iter_mut().find(|f| f.id == id.as_str()) {
                f.enabled = enabled;
                let _ = cfg.save();
                if let Some(m) = m_weak.upgrade() {
                    m.set_folders(make_folders_model(&cfg.folders));
                }
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_remove_folder(move |id| {
            let mut cfg = config_clone.lock().unwrap();
            cfg.folders.retain(|f| f.id != id.as_str());
            let _ = cfg.save();
            if let Some(m) = m_weak.upgrade() {
                m.set_folders(make_folders_model(&cfg.folders));
            }
        });
    }

    main_window.on_open_folder_path(|path_str| {
        let _ = std::process::Command::new("explorer")
            .arg(path_str.as_str())
            .spawn();
    });

    {
        let m_weak = main_window.as_weak();
        let registry_for_browse = registry.clone();
        main_window.on_browse_file_clicked(move || {
            if let Some(file_path) = pick_file(None) {
                if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                    if let Some(fmt) = FileFormat::from_extension(ext) {
                        let name = file_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let targets = registry_for_browse
                            .get_available_targets_for_file(fmt, Some(&file_path));
                        let default_target = targets
                            .first()
                            .map(|f| f.display_label())
                            .unwrap_or("PNG");

                        if let Some(m) = m_weak.upgrade() {
                            m.set_convert_source_path(file_path.display().to_string().into());
                            m.set_convert_source_name(name.into());
                            m.set_convert_source_ext(fmt.display_label().into());
                            m.set_convert_target_fmt(default_target.into());
                            m.set_convert_progress(0.0);
                            m.set_is_converting(false);
                            m.set_convert_status_msg(
                                format!("Ready to convert to {}", default_target).into(),
                            );
                        }
                    }
                }
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        main_window.on_select_target_format(move |fmt_str| {
            if let Some(m) = m_weak.upgrade() {
                m.set_convert_target_fmt(fmt_str.clone());
                m.set_convert_status_msg(format!("Ready to convert to {}", fmt_str).into());
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let job_mgr_for_manual = job_manager.clone();
        let config_for_manual = config.clone();
        let active_id_for_manual = active_conversion_id.clone();
        main_window.on_start_manual_conversion(move || {
            if let Some(m) = m_weak.upgrade() {
                let src_path_str = m.get_convert_source_path().to_string();
                if src_path_str.is_empty() {
                    return;
                }
                let src_path = PathBuf::from(&src_path_str);
                let ext = src_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default();
                let src_fmt = match FileFormat::from_extension(ext) {
                    Some(f) => f,
                    None => return,
                };
                let target_str = m.get_convert_target_fmt().to_string();
                let target_fmt = match FileFormat::from_extension(&target_str) {
                    Some(f) => f,
                    None => FileFormat::Png,
                };

                m.set_is_converting(true);
                m.set_convert_progress(0.0);
                m.set_convert_status_msg("Converting ... 0%".into());

                let custom_out = {
                    let cfg = config_for_manual.lock().unwrap();
                    cfg.custom_output_dir.clone()
                };

                info!(
                    source = %src_path.display(),
                    target = %target_fmt,
                    "Starting manual conversion from Dashboard"
                );

                if let Ok(job_id) = job_mgr_for_manual.submit(src_path, src_fmt, target_fmt, custom_out) {
                    *active_id_for_manual.lock().unwrap() = Some(job_id);
                }
            }
        });
    }

    {
        let job_mgr_for_cancel = job_manager.clone();
        let active_id_for_cancel = active_conversion_id.clone();
        let m_weak = main_window.as_weak();
        main_window.on_cancel_conversion(move || {
            info!("Conversion cancellation requested from UI");
            if let Some(id) = *active_id_for_cancel.lock().unwrap() {
                job_mgr_for_cancel.cancel(id);
            }
            job_mgr_for_cancel.cancel_all();
            if let Some(m) = m_weak.upgrade() {
                m.set_is_converting(false);
                m.set_convert_status_msg("Conversion cancelled".into());
            }
        });
    }

    {
        let job_mgr_for_retry = job_manager.clone();
        let active_id_for_retry = active_conversion_id.clone();
        let m_weak = main_window.as_weak();
        main_window.on_retry_conversion(move |hist_id| {
            info!(hist_id, "Conversion retry requested from History UI");
            match job_mgr_for_retry.retry(hist_id as u64) {
                Ok(new_id) => {
                    *active_id_for_retry.lock().unwrap() = Some(new_id);
                    if let Some(m) = m_weak.upgrade() {
                        m.set_is_converting(true);
                        m.set_convert_progress(0.0);
                        m.set_convert_status_msg("Retrying conversion ... 0%".into());
                    }
                }
                Err(err) => {
                    warn!("Failed to retry job #{}: {}", hist_id, err);
                }
            }
        });
    }

    main_window.on_open_output_file(|path_str| {
        let _ = std::process::Command::new("explorer")
            .arg(format!("/select,{}", path_str.as_str()))
            .spawn();
    });

    {
        let m_weak = main_window.as_weak();
        let hist_for_clear = history_store.clone();
        main_window.on_clear_history_clicked(move || {
            hist_for_clear.lock().unwrap().clear();
            if let Some(m) = m_weak.upgrade() {
                m.set_history(ModelRc::new(VecModel::from(Vec::<HistoryItem>::new())));
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_apply_preset_clicked(move |preset_id| {
            let cfg = config_clone.lock().unwrap();
            if let Some(p) = cfg.presets.iter().find(|p| p.id == preset_id.as_str()) {
                if let Some(m) = m_weak.upgrade() {
                    m.set_convert_target_fmt(p.target_format.clone().into());
                    m.set_jpeg_quality(p.quality as i32);
                    m.set_svg_scale(p.svg_scale);
                    m.set_selected_nav(1); // Jump to Convert tab
                }
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_save_settings_clicked(move || {
            if let Some(m) = m_weak.upgrade() {
                let mut cfg = config_clone.lock().unwrap();
                cfg.jpeg_quality = m.get_jpeg_quality() as u8;
                cfg.svg_scale = m.get_svg_scale();
                cfg.show_notifications = m.get_show_notifications();
                cfg.launch_on_startup = m.get_launch_on_startup();
                cfg.minimize_to_tray = m.get_minimize_to_tray();
                cfg.hotkey_wheel_a = m.get_hotkey_wheel_a().to_string();
                cfg.hotkey_wheel_b = m.get_hotkey_wheel_b().to_string();
                cfg.hotkey_command_palette = m.get_hotkey_command_palette().to_string();
                let _ = cfg.save();
                orbit_windows::configure_hotkeys(orbit_windows::HotkeySettings {
                    wheel_a: cfg.hotkey_wheel_a.clone(),
                    wheel_b: cfg.hotkey_wheel_b.clone(),
                    command_palette: cfg.hotkey_command_palette.clone(),
                });
                info!("Preferences saved successfully: Wheel A = {}, Wheel B = {}, Command Palette = {}",
                    cfg.hotkey_wheel_a, cfg.hotkey_wheel_b, cfg.hotkey_command_palette);
                tray::send_global_notification(
                    "OrbitConvert Preferences",
                    &format!(
                        "Settings saved.\n• Wheel A: {}\n• Wheel B: {}\n• Command Palette: {}",
                        cfg.hotkey_wheel_a, cfg.hotkey_wheel_b, cfg.hotkey_command_palette
                    ),
                    false,
                );
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_reset_hotkeys_to_default(move || {
            if let Some(m) = m_weak.upgrade() {
                let mut cfg = config_clone.lock().unwrap();
                cfg.reset_hotkeys_to_default();
                let _ = cfg.save();
                m.set_hotkey_wheel_a(cfg.hotkey_wheel_a.clone().into());
                m.set_hotkey_wheel_b(cfg.hotkey_wheel_b.clone().into());
                m.set_hotkey_command_palette(cfg.hotkey_command_palette.clone().into());
                orbit_windows::configure_hotkeys(orbit_windows::HotkeySettings {
                    wheel_a: cfg.hotkey_wheel_a.clone(),
                    wheel_b: cfg.hotkey_wheel_b.clone(),
                    command_palette: cfg.hotkey_command_palette.clone(),
                });
                info!("Hotkeys restored to defaults: Wheel A = {}, Wheel B = {}, Command Palette = {}",
                    cfg.hotkey_wheel_a, cfg.hotkey_wheel_b, cfg.hotkey_command_palette);
                tray::send_global_notification(
                    "OrbitConvert",
                    "Hotkeys restored to defaults:\n• Wheel A: Hold Alt\n• Wheel B: Hold ` (Backtick)\n• Command Palette: Ctrl+Shift+Space",
                    false,
                );
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let config_clone = config.clone();
        main_window.on_choose_custom_output_clicked(move || {
            if let Some(folder_path) = pick_folder(None) {
                let mut cfg = config_clone.lock().unwrap();
                cfg.custom_output_dir = Some(folder_path.clone());
                let _ = cfg.save();
                if let Some(m) = m_weak.upgrade() {
                    m.set_output_mode("custom".into());
                    m.set_output_folder_label(folder_path.display().to_string().into());
                }
            }
        });
    }

    {
        let config_clone = config.clone();
        main_window.on_hotkey_selected(move |wheel_a, wheel_b, palette| {
            let mut cfg = config_clone.lock().unwrap();
            cfg.hotkey_wheel_a = wheel_a.to_string();
            cfg.hotkey_wheel_b = wheel_b.to_string();
            cfg.hotkey_command_palette = palette.to_string();
            let _ = cfg.save();
            orbit_windows::configure_hotkeys(orbit_windows::HotkeySettings {
                wheel_a: cfg.hotkey_wheel_a.clone(),
                wheel_b: cfg.hotkey_wheel_b.clone(),
                command_palette: cfg.hotkey_command_palette.clone(),
            });
            info!("Live hotkeys updated: Wheel A = {}, Wheel B = {}, Command Palette = {}",
                cfg.hotkey_wheel_a, cfg.hotkey_wheel_b, cfg.hotkey_command_palette);
            tray::send_global_notification(
                "OrbitConvert Hotkeys",
                &format!("Hotkeys configured live:\n• Conversion Wheel: {}\n• Quick Actions: {}\n• Palette: {}",
                    cfg.hotkey_wheel_a, cfg.hotkey_wheel_b, cfg.hotkey_command_palette
                ),
                false,
            );
        });
    }

    {
        let config_clone = config.clone();
        let m_weak = main_window.as_weak();
        main_window.on_reset_default_folders_clicked(move || {
            let mut cfg = config_clone.lock().unwrap();
            cfg.reset_folders_to_default();
            let _ = cfg.save();
            let updated = cfg.folders.clone();
            if let Some(m) = m_weak.upgrade() {
                m.set_folders(make_folders_model(&updated));
            }
            tray::send_global_notification(
                "OrbitConvert Folders",
                "Default high-traffic folders (Desktop, Downloads, Documents, Pictures) restored.",
                false,
            );
        });
    }

    {
        let config_clone = config.clone();
        let m_weak = main_window.as_weak();
        main_window.on_dismiss_onboarding(move || {
            let mut cfg = config_clone.lock().unwrap();
            cfg.show_onboarding = false;
            let _ = cfg.save();
            if let Some(m) = m_weak.upgrade() {
                m.set_show_onboarding(false);
            }
        });
    }

    {
        let config_clone = config.clone();
        let m_weak = main_window.as_weak();
        main_window.on_toggle_onboarding(move || {
            let mut cfg = config_clone.lock().unwrap();
            cfg.show_onboarding = !cfg.show_onboarding;
            let _ = cfg.save();
            if let Some(m) = m_weak.upgrade() {
                m.set_show_onboarding(cfg.show_onboarding);
            }
        });
    }

    {
        let resolver_for_diag = resolver.clone();
        let m_weak = main_window.as_weak();
        main_window.on_run_diagnostics_clicked(move || {
            if let Some(m) = m_weak.upgrade() {
                m.set_is_running_diagnostics(true);
            }
            if let Ok(pt) = get_current_cursor_pos() {
                let report = resolver_for_diag.diagnose_cursor(pt);
                print_diagnostic_report(&report);

                let now = std::time::SystemTime::now();
                let time_str = match now.duration_since(std::time::UNIX_EPOCH) {
                    Ok(d) => {
                        let secs = d.as_secs();
                        let h = (secs / 3600) % 24;
                        let m = (secs / 60) % 60;
                        let s = secs % 60;
                        format!("{:02}:{:02}:{:02} UTC", h, m, s)
                    }
                    Err(_) => "Just now".to_string(),
                };

                if let Some(m) = m_weak.upgrade() {
                    m.set_diag_window_class(report.window_class.clone().into());
                    let display_el_name = match (&report.clean_element_name, &report.uia_element_name) {
                        (Some(c), Some(r)) if c != r => format!("{} (raw: {})", c, r),
                        (Some(c), _) => c.clone(),
                        (None, Some(r)) => r.clone(),
                        (None, None) => "None".to_string(),
                    };
                    m.set_diag_element_name(display_el_name.into());
                    m.set_diag_resolved_path(
                        report
                            .resolved_item_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "None".to_string())
                            .into(),
                    );
                    m.set_diag_detected_format(
                        report
                            .detected_format
                            .map(|f| f.display_label().to_string())
                            .unwrap_or_else(|| "None".to_string())
                            .into(),
                    );
                    m.set_diag_status(
                        report
                            .rejection_reason
                            .clone()
                            .unwrap_or_else(|| "Supported & Ready to Convert".to_string())
                            .into(),
                    );
                    m.set_diag_timestamp(time_str.into());
                    m.set_is_running_diagnostics(false);
                }

                let title = if report.rejection_reason.is_none() {
                    "OrbitConvert: Supported File"
                } else {
                    "OrbitConvert: Diagnostic Report"
                };
                let desc = format!(
                    "Class: {} | Element: {:?}\nStatus: {}",
                    report.window_class,
                    report.uia_element_name,
                    report.rejection_reason.as_deref().unwrap_or("Ready to convert")
                );
                tray::send_global_notification(title, &desc, false);
            } else if let Some(m) = m_weak.upgrade() {
                m.set_is_running_diagnostics(false);
            }
        });
    }

    // -------------------------------------------------------------
    // Intercept Window Close: Minimize to System Tray
    // -------------------------------------------------------------
    {
        let m_weak = main_window.as_weak();
        let config_for_close = config.clone();
        main_window.window().on_close_requested(move || {
            let show_notifications = {
                let cfg = config_for_close.lock().unwrap();
                cfg.show_notifications
            };

            info!("MainWindow close requested by user: minimizing to system tray instead of closing");
            if let Some(m) = m_weak.upgrade() {
                let _ = m.hide();
            }

            if show_notifications {
                tray::send_global_notification(
                    "OrbitConvert Running in Background",
                    "OrbitConvert is minimized to the system tray. Hold Alt or backtick (`) to convert files.",
                    false,
                );
            }

            slint::CloseRequestResponse::HideWindow
        });
    }

    // -------------------------------------------------------------
    // System Tray Menu Actions
    // -------------------------------------------------------------
    let resolver_for_tray = resolver.clone();
    let main_weak_for_tray = main_window.as_weak();
    let is_enabled_flag = tray.is_enabled.clone();
    let config_for_tray = config.clone();

    tray::set_tray_action_handler(move |action_id| match action_id {
        tray::ID_TRAY_DASHBOARD => {
            if let Some(m) = main_weak_for_tray.upgrade() {
                let _ = m.show();
                std::thread::spawn(|| {
                    tray::apply_window_icon();
                });
            }
        }
        tray::ID_TRAY_SETTINGS => {
            if let Some(m) = main_weak_for_tray.upgrade() {
                m.set_selected_nav(5);
                let _ = m.show();
                std::thread::spawn(|| {
                    tray::apply_window_icon();
                });
            }
        }
        tray::ID_TRAY_STATUS => {
            let current = is_enabled_flag.load(Ordering::Relaxed);
            is_enabled_flag.store(!current, Ordering::Relaxed);
            if let Some(m) = main_weak_for_tray.upgrade() {
                m.set_service_active(!current);
            }
            info!("OrbitConvert toggled via tray. Active: {}", !current);
        }
        tray::ID_TRAY_DIAGNOSE => {
            if let Ok(pt) = get_current_cursor_pos() {
                let report = resolver_for_tray.diagnose_cursor(pt);
                print_diagnostic_report(&report);
            }
        }
        tray::ID_TRAY_OPEN_DIR => {
            let cfg = config_for_tray.lock().unwrap();
            let target_dir = cfg
                .custom_output_dir
                .as_deref()
                .unwrap_or_else(|| std::path::Path::new("."));
            let _ = std::process::Command::new("explorer").arg(target_dir).spawn();
        }
        tray::ID_TRAY_EXIT => {
            info!("Exit requested from tray menu");
            std::process::exit(0);
        }
        _ => {}
    });

    // Check CLI test argument: --test-wheel
    if args.iter().any(|arg| arg == "--test-wheel") {
        info!("Running --test-wheel demonstration");
        let (inner_sec, outer_sec, _, _) = build_two_level_wheel_sectors(
            230.0,
            230.0,
            &[
                FileFormat::Png,
                FileFormat::Jpg,
                FileFormat::Webp,
                FileFormat::Ico,
            ],
            false,
            &[],
        );
        wheel.set_source_ext("SVG".into());
        wheel.set_source_name("test_logo.svg".into());
        wheel.set_inner_sectors(inner_sec);
        wheel.set_outer_sectors(outer_sec);
        wheel.set_is_expanded(false);
        wheel.set_highlighted_inner_index(0);
        wheel.set_highlighted_outer_index(-1);
        wheel.set_status_text("Test wheel mode (auto-closing in 4s)".into());
        wheel.window().set_position(LogicalPosition::new(300.0, 300.0));
        let _ = wheel.show();

        let wheel_weak = wheel.as_weak();
        slint::Timer::single_shot(Duration::from_secs(4), move || {
            if let Some(w) = wheel_weak.upgrade() {
                let _ = w.hide();
                std::process::exit(0);
            }
        });
    }

    if args.iter().any(|arg| arg == "--test-actions-wheel") {
        info!("Running --test-actions-wheel demonstration");
        let (inner_acts, outer_acts) = orbit_core::actions::get_quick_actions_for_format(FileFormat::Png);
        let (inner_sec, outer_sec, _, _) = orbit_ui::build_two_level_action_sectors(
            230.0,
            230.0,
            &inner_acts,
            !outer_acts.is_empty(),
            &outer_acts,
        );
        wheel.set_wheel_mode("ACTIONS".into());
        wheel.set_source_ext("PNG".into());
        wheel.set_source_name("product_photo.png".into());
        wheel.set_inner_sectors(inner_sec);
        wheel.set_outer_sectors(outer_sec);
        wheel.set_is_expanded(false);
        wheel.set_highlighted_inner_index(0);
        wheel.set_highlighted_outer_index(-1);
        wheel.set_status_text("Move cursor to action, release ` to execute".into());
        wheel.window().set_position(LogicalPosition::new(300.0, 300.0));
        let _ = wheel.show();

        let wheel_weak = wheel.as_weak();
        slint::Timer::single_shot(Duration::from_secs(4), move || {
            if let Some(w) = wheel_weak.upgrade() {
                let _ = w.hide();
                std::process::exit(0);
            }
        });
    }

    // Show MainWindow dashboard on startup unless --minimized is specified
    if !args.iter().any(|arg| arg == "--minimized") {
        let _ = main_window.show();
        std::thread::spawn(|| {
            tray::apply_window_icon();
        });
    }

    // -------------------------------------------------------------
    // Main Interaction Loop (Radial Wheel triggered by Alt key)
    // -------------------------------------------------------------
    let wheel_weak = wheel.as_weak();
    let resolver_clone = resolver.clone();
    let registry_clone = registry.clone();
    let job_mgr_clone = job_manager.clone();
    let is_enabled_for_loop = tray.is_enabled.clone();
    let config_for_wheel = config.clone();

    // MainWindow Preview Wheel callback (accessible from Dashboard & Settings)
    {
        let wheel_for_preview = wheel.as_weak();
        let interaction_for_preview = interaction.clone();
        main_window.on_preview_wheel_clicked(move || {
            if let Some(w) = wheel_for_preview.upgrade() {
                let demo_inner = vec![
                    FileFormat::Png,
                    FileFormat::Jpg,
                    FileFormat::Webp,
                    FileFormat::Ico,
                    FileFormat::Bmp,
                ];
                let demo_outer = vec![
                    FileFormat::Gif,
                    FileFormat::Tiff,
                    FileFormat::Tga,
                    FileFormat::Avif,
                    FileFormat::Heic,
                    FileFormat::Jxl,
                ];
                let (inner_sec, outer_sec, inner_targets, outer_targets) =
                    build_two_level_wheel_sectors(230.0, 230.0, &demo_inner, true, &demo_outer);
                w.set_source_ext("SVG".into());
                w.set_source_name("sample_vector.svg".into());
                w.set_inner_sectors(inner_sec);
                w.set_outer_sectors(outer_sec);
                w.set_is_expanded(false);
                w.set_highlighted_inner_index(-1);
                w.set_highlighted_outer_index(-1);
                w.set_status_text("Interactive Preview (Move toward More to expand, Esc to close)".into());
                w.set_is_preview(true);

                let cur_pos = get_current_cursor_pos().unwrap_or(Point::new(600.0, 400.0));
                let scale_factor = get_monitor_scale_factor(cur_pos);
                let (wx, wy, phys_center) = calculate_clamped_window_pos_physical(
                    cur_pos, 460.0, 460.0, scale_factor,
                );
                w.window().set_position(PhysicalPosition::new(wx, wy));
                let _ = w.show();

                let offset_from_center = cur_pos.distance_to(&phys_center) / scale_factor;
                if offset_from_center > 60.0 {
                    let _ = set_current_cursor_pos(phys_center);
                }

                let mut data = interaction_for_preview.lock().unwrap();
                data.state = AppState::WheelActive;
                data.wheel_kind = WheelKind::Convert;
                data.center_pos = phys_center;
                data.scale_factor = scale_factor;
                data.source_path = None;
                data.source_format = Some(FileFormat::Svg);
                data.inner_targets = inner_targets;
                data.outer_targets = outer_targets;
                data.has_more = true;
                data.is_expanded = false;
                data.current_highlighted_inner = -1;
                data.current_highlighted_outer = -1;
                data.is_preview = true;
            }
        });
    }

    // All Formats Dialog handlers
    {
        let m_weak = main_window.as_weak();
        let cap_matrix = registry.get_capability_matrix();
        main_window.on_open_all_formats_dialog(move || {
            if let Some(m) = m_weak.upgrade() {
                m.set_format_search_filter("".into());
                m.set_capability_matrix(make_capability_matrix_model(&cap_matrix, ""));
                m.set_show_all_formats_modal(true);
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let cap_matrix = registry.get_capability_matrix();
        main_window.on_format_search_changed(move |query| {
            if let Some(m) = m_weak.upgrade() {
                m.set_capability_matrix(make_capability_matrix_model(&cap_matrix, query.as_str()));
            }
        });
    }

    {
        let m_weak = main_window.as_weak();
        let wheel_weak = wheel.as_weak();
        let cap_matrix = registry.get_capability_matrix();
        let interaction_for_all = interaction.clone();
        wheel.on_open_all_formats_requested(move || {
            if let Some(w) = wheel_weak.upgrade() {
                let _ = w.hide();
            }
            {
                let mut data = interaction_for_all.lock().unwrap();
                data.state = AppState::Idle;
                data.current_highlighted_inner = -1;
                data.current_highlighted_outer = -1;
                data.is_expanded = false;
            }
            if let Some(m) = m_weak.upgrade() {
                let _ = m.show();
                m.set_selected_nav(1); // Jump to Convert Studio
                m.set_format_search_filter("".into());
                m.set_capability_matrix(make_capability_matrix_model(&cap_matrix, ""));
                m.set_show_all_formats_modal(true);
            }
        });
    }

    {
        let wheel_for_click = wheel.as_weak();
        let interaction_for_click = interaction.clone();
        let job_mgr_for_click = job_manager.clone();
        let config_for_click = config.clone();
        let active_id_for_click = active_conversion_id.clone();
        wheel.on_sector_selected(move |idx, is_outer| {
            let mut data = interaction_for_click.lock().unwrap();
            if data.is_preview {
                return;
            }
            if data.wheel_kind == WheelKind::Convert {
                let target_fmt = if is_outer {
                    data.outer_targets.get(idx as usize).copied()
                } else {
                    if (idx as usize) < data.inner_targets.len() {
                        data.inner_targets.get(idx as usize).copied()
                    } else {
                        // Clicked More
                        data.is_expanded = true;
                        if let Some(w) = wheel_for_click.upgrade() {
                            w.set_is_expanded(true);
                        }
                        return;
                    }
                };

                if let (Some(target_fmt), Some(src_path), Some(src_fmt)) =
                    (target_fmt, &data.source_path, data.source_format)
                {
                    let custom_out = {
                        let cfg = config_for_click.lock().unwrap();
                        cfg.custom_output_dir.clone()
                    };
                    if let Some(w) = wheel_for_click.upgrade() {
                        w.set_status_text("Converting ... 0%".into());
                    }
                    if let Ok(job_id) = job_mgr_for_click.submit(src_path.clone(), src_fmt, target_fmt, custom_out) {
                        *active_id_for_click.lock().unwrap() = Some(job_id);
                    }
                    data.state = AppState::Converting;
                }
            } else {
                // WheelKind::QuickActions
                let action = if is_outer {
                    data.outer_actions.get(idx as usize).copied()
                } else {
                    if (idx as usize) < data.inner_actions.len() {
                        data.inner_actions.get(idx as usize).copied()
                    } else {
                        // Clicked More Actions
                        data.is_expanded = true;
                        if let Some(w) = wheel_for_click.upgrade() {
                            w.set_is_expanded(true);
                        }
                        return;
                    }
                };

                if let (Some(act), Some(src_path), Some(src_fmt)) =
                    (action, &data.source_path, data.source_format)
                {
                    let show_notifs = config_for_click.lock().unwrap().show_notifications;
                    quick_actions_exec::execute_quick_action(
                        act,
                        src_path,
                        src_fmt,
                        &job_mgr_for_click,
                        show_notifs,
                    );
                    if let Some(w) = wheel_for_click.upgrade() {
                        let _ = w.hide();
                    }
                    data.state = AppState::Idle;
                    data.current_highlighted_inner = -1;
                    data.current_highlighted_outer = -1;
                    data.is_expanded = false;
                }
            }
        });
    }

    {
        let wheel_for_close = wheel.as_weak();
        let interaction_for_close = interaction.clone();
        wheel.on_close_requested(move || {
            if let Some(w) = wheel_for_close.upgrade() {
                let _ = w.hide();
                w.set_is_preview(false);
            }
            let mut data = interaction_for_close.lock().unwrap();
            data.state = AppState::Idle;
            data.current_highlighted_inner = -1;
            data.current_highlighted_outer = -1;
            data.is_expanded = false;
            data.is_preview = false;
        });
    }

    let timer = slint::Timer::default();
    let interaction_clone = interaction.clone();
    let active_id_for_loop = active_conversion_id.clone();
    let main_weak_for_loop = main_window.as_weak();
    let config_for_loop = config.clone();

    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            // Process incoming keyboard hook events
            while let Ok(event) = key_rx.try_recv() {
                if !is_enabled_for_loop.load(Ordering::Relaxed) {
                    continue;
                }

                match event {
                    RawKeyEvent::AltDown => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::Idle {
                            let cursor_pos = match get_current_cursor_pos() {
                                Ok(pos) => pos,
                                Err(e) => {
                                    debug!("Could not get cursor position on AltDown: {}", e);
                                    continue;
                                }
                            };

                            // Check whether cursor or foreground belongs to Explorer/Desktop
                            if !is_cursor_or_foreground_explorer(cursor_pos.x, cursor_pos.y) {
                                debug!(
                                    x = cursor_pos.x,
                                    y = cursor_pos.y,
                                    "Alt pressed, but cursor/foreground window is not Explorer or Desktop"
                                );
                                continue;
                            }

                            // Run cursor diagnosis
                            let report = resolver_clone.diagnose_cursor(cursor_pos);
                            info!(
                                class = %report.window_class,
                                is_explorer = report.is_explorer_or_desktop,
                                element = ?report.uia_element_name,
                                clean_element = ?report.clean_element_name,
                                path = ?report.resolved_item_path,
                                fmt = ?report.detected_format,
                                reason = ?report.rejection_reason,
                                "Explorer item inspection on AltDown"
                            );

                            // Always update Diagnostics tab state so user sees real-time status
                            if let Some(m) = main_weak_for_loop.upgrade() {
                                m.set_diag_window_class(report.window_class.clone().into());
                                let display_el_name = match (&report.clean_element_name, &report.uia_element_name) {
                                    (Some(c), Some(r)) if c != r => format!("{} (raw: {})", c, r),
                                    (Some(c), _) => c.clone(),
                                    (None, Some(r)) => r.clone(),
                                    (None, None) => "None".to_string(),
                                };
                                m.set_diag_element_name(display_el_name.into());
                                m.set_diag_resolved_path(
                                    report.resolved_item_path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "None".to_string()).into(),
                                );
                                m.set_diag_detected_format(
                                    report.detected_format.map(|f| f.display_label().to_string()).unwrap_or_else(|| "None".to_string()).into(),
                                );
                                let status_str = if let Some(ref r) = report.rejection_reason {
                                    r.clone()
                                } else if let Some(fmt) = report.detected_format {
                                    let targets = registry_clone.get_available_targets_for_file(fmt, report.resolved_item_path.as_deref());
                                    if targets.is_empty() {
                                        format!("Resolved as {} (No conversion targets registered yet)", fmt.display_label())
                                    } else {
                                        format!("Ready to Convert ({} targets available)", targets.len())
                                    }
                                } else {
                                    "No file resolved under cursor".to_string()
                                };
                                m.set_diag_status(status_str.into());
                            }

                            if let (Some(ref item_path), Some(src_fmt)) =
                                (&report.resolved_item_path, report.detected_format)
                            {
                                let (inner_fmts, outer_fmts) =
                                    registry_clone.get_default_and_expanded_targets_for_file(
                                        src_fmt,
                                        Some(item_path.as_path()),
                                    );
                                if inner_fmts.is_empty() {
                                    warn!(
                                        format = %src_fmt,
                                        path = %item_path.display(),
                                        "Supported file detected but no target conversions available in registry"
                                    );
                                    tray::update_global_tooltip(&format!(
                                        "OrbitConvert: {} (No registered conversion routes)",
                                        src_fmt.display_label()
                                    ));
                                    continue;
                                }

                                info!(
                                    file = %item_path.display(),
                                    format = %src_fmt,
                                    inner = ?inner_fmts,
                                    outer = ?outer_fmts,
                                    "File resolved with valid conversion routes. Showing two-level radial wheel."
                                );

                                let has_more = !outer_fmts.is_empty();
                                let (inner_sec, outer_sec, inner_targets, outer_targets) =
                                    build_two_level_wheel_sectors(
                                        230.0, 230.0, &inner_fmts, has_more, &outer_fmts,
                                    );
                                let fname = item_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                let scale_factor = get_monitor_scale_factor(cursor_pos);
                                let (wx, wy, phys_center) = calculate_clamped_window_pos_physical(
                                    cursor_pos, 460.0, 460.0, scale_factor,
                                );
                                let offset_from_center = cursor_pos.distance_to(&phys_center) / scale_factor;
                                if offset_from_center > 60.0 {
                                    let _ = set_current_cursor_pos(phys_center);
                                }

                                data.state = AppState::WheelActive;
                                data.wheel_kind = WheelKind::Convert;
                                data.center_pos = phys_center;
                                data.scale_factor = scale_factor;
                                data.source_path = Some(item_path.clone());
                                data.source_format = Some(src_fmt);
                                data.inner_targets = inner_targets;
                                data.outer_targets = outer_targets;
                                data.inner_actions.clear();
                                data.outer_actions.clear();
                                data.has_more = has_more;
                                data.is_expanded = false;
                                data.current_highlighted_inner = -1;
                                data.current_highlighted_outer = -1;
                                data.is_preview = false;

                                if let Some(w) = wheel_weak.upgrade() {
                                    w.set_is_preview(false);
                                    w.set_wheel_mode("CONVERT".into());
                                    w.set_source_ext(src_fmt.display_label().into());
                                    w.set_source_name(fname.into());
                                    w.set_inner_sectors(inner_sec);
                                    w.set_outer_sectors(outer_sec);
                                    w.set_is_expanded(false);
                                    w.set_highlighted_inner_index(-1);
                                    w.set_highlighted_outer_index(-1);
                                    let hotkey_a = config_for_loop.lock().unwrap().hotkey_wheel_a.clone();
                                    let key_label = match hotkey_a.as_str() {
                                        "Hold Ctrl" => "Ctrl",
                                        "Hold Shift" => "Shift",
                                        "Hold CapsLock" => "CapsLock",
                                        _ => "Alt",
                                    };
                                    w.set_status_text(
                                        format!("Move cursor to format, release {} to convert", key_label).into(),
                                    );

                                    w.window().set_position(PhysicalPosition::new(wx, wy));
                                    let _ = w.show();
                                }
                            }
                        }
                    }
                    RawKeyEvent::AltUp => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::WheelActive && data.wheel_kind == WheelKind::Convert {
                            if !data.is_preview {
                                let target_fmt: Option<FileFormat> = if data.current_highlighted_outer >= 0 {
                                    let idx = data.current_highlighted_outer as usize;
                                    data.outer_targets.get(idx).copied()
                                } else if data.current_highlighted_inner >= 0 {
                                    let idx = data.current_highlighted_inner as usize;
                                    if idx < data.inner_targets.len() {
                                        data.inner_targets.get(idx).copied()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if let Some(target_fmt) = target_fmt {
                                    if let (Some(src_path), Some(src_fmt)) =
                                        (&data.source_path, data.source_format)
                                    {
                                        info!(
                                            source = %src_path.display(),
                                            target = %target_fmt,
                                            "Radial conversion initiated by Alt release"
                                        );

                                        let custom_out = {
                                            let cfg = config_for_wheel.lock().unwrap();
                                            cfg.custom_output_dir.clone()
                                        };

                                        if let Some(w) = wheel_weak.upgrade() {
                                            w.set_status_text("Converting ... 0%".into());
                                        }

                                        if let Ok(job_id) = job_mgr_clone.submit(
                                            src_path.clone(),
                                            src_fmt,
                                            target_fmt,
                                            custom_out,
                                        ) {
                                            *active_id_for_loop.lock().unwrap() = Some(job_id);
                                        }

                                        data.state = AppState::Converting;
                                        continue;
                                    }
                                } else {
                                    debug!("Alt released in dead zone or outside; conversion cancelled");
                                }
                            }

                            if let Some(w) = wheel_weak.upgrade() {
                                let _ = w.hide();
                            }
                            data.state = AppState::Idle;
                            data.current_highlighted_inner = -1;
                            data.current_highlighted_outer = -1;
                            data.is_expanded = false;
                        }
                    }
                    RawKeyEvent::BacktickDown => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::Idle {
                            let cursor_pos = match get_current_cursor_pos() {
                                Ok(pos) => pos,
                                Err(e) => {
                                    debug!("Could not get cursor position on BacktickDown: {}", e);
                                    continue;
                                }
                            };

                            // Check whether cursor or foreground belongs to Explorer/Desktop
                            if !is_cursor_or_foreground_explorer(cursor_pos.x, cursor_pos.y) {
                                debug!(
                                    x = cursor_pos.x,
                                    y = cursor_pos.y,
                                    "Backtick pressed, but cursor/foreground window is not Explorer or Desktop"
                                );
                                continue;
                            }

                            // Run cursor diagnosis
                            let report = resolver_clone.diagnose_cursor(cursor_pos);
                            info!(
                                class = %report.window_class,
                                is_explorer = report.is_explorer_or_desktop,
                                element = ?report.clean_element_name,
                                path = ?report.resolved_item_path,
                                fmt = ?report.detected_format,
                                "Explorer item inspection on BacktickDown (Quick Actions Wheel)"
                            );

                            if let (Some(ref item_path), Some(src_fmt)) =
                                (&report.resolved_item_path, report.detected_format)
                            {
                                let (inner_actions, outer_actions) = get_quick_actions_for_format(src_fmt);
                                let has_more = !outer_actions.is_empty();
                                let (inner_sec, outer_sec, inner_acts, outer_acts) =
                                    build_two_level_action_sectors(
                                        230.0, 230.0, &inner_actions, has_more, &outer_actions,
                                    );
                                let fname = item_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default();

                                let scale_factor = get_monitor_scale_factor(cursor_pos);
                                let (wx, wy, phys_center) = calculate_clamped_window_pos_physical(
                                    cursor_pos, 460.0, 460.0, scale_factor,
                                );
                                let offset_from_center = cursor_pos.distance_to(&phys_center) / scale_factor;
                                if offset_from_center > 60.0 {
                                    let _ = set_current_cursor_pos(phys_center);
                                }

                                data.state = AppState::WheelActive;
                                data.wheel_kind = WheelKind::QuickActions;
                                data.center_pos = phys_center;
                                data.scale_factor = scale_factor;
                                data.source_path = Some(item_path.clone());
                                data.source_format = Some(src_fmt);
                                data.inner_targets.clear();
                                data.outer_targets.clear();
                                data.inner_actions = inner_acts;
                                data.outer_actions = outer_acts;
                                data.has_more = has_more;
                                data.is_expanded = false;
                                data.current_highlighted_inner = -1;
                                data.current_highlighted_outer = -1;
                                data.is_preview = false;

                                if let Some(w) = wheel_weak.upgrade() {
                                    w.set_is_preview(false);
                                    w.set_wheel_mode("ACTIONS".into());
                                    w.set_source_ext(src_fmt.display_label().into());
                                    w.set_source_name(fname.into());
                                    w.set_inner_sectors(inner_sec);
                                    w.set_outer_sectors(outer_sec);
                                    w.set_is_expanded(false);
                                    w.set_highlighted_inner_index(-1);
                                    w.set_highlighted_outer_index(-1);
                                    let hotkey_b = config_for_loop.lock().unwrap().hotkey_wheel_b.clone();
                                    let key_label = match hotkey_b.as_str() {
                                        "Hold Tab" => "Tab",
                                        "Hold Q" => "Q",
                                        "Hold X" => "X",
                                        "Hold C" => "C",
                                        _ => "`",
                                    };
                                    w.set_status_text(
                                        format!("Move cursor to action, release {} to execute", key_label).into(),
                                    );

                                    w.window().set_position(PhysicalPosition::new(wx, wy));
                                    let _ = w.show();
                                }
                            }
                        }
                    }
                    RawKeyEvent::BacktickUp => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::WheelActive && data.wheel_kind == WheelKind::QuickActions {
                            if !data.is_preview {
                                let selected_action: Option<QuickAction> = if data.current_highlighted_outer >= 0 {
                                    let idx = data.current_highlighted_outer as usize;
                                    data.outer_actions.get(idx).copied()
                                } else if data.current_highlighted_inner >= 0 {
                                    let idx = data.current_highlighted_inner as usize;
                                    if idx < data.inner_actions.len() {
                                        data.inner_actions.get(idx).copied()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                if let Some(act) = selected_action {
                                    if let (Some(src_path), Some(src_fmt)) =
                                        (&data.source_path, data.source_format)
                                    {
                                        info!(
                                            source = %src_path.display(),
                                            action = ?act,
                                            "Quick Action initiated by backtick release"
                                        );
                                        let show_notifs = config_for_wheel.lock().unwrap().show_notifications;
                                        quick_actions_exec::execute_quick_action(
                                            act,
                                            src_path,
                                            src_fmt,
                                            &job_mgr_clone,
                                            show_notifs,
                                        );
                                    }
                                } else {
                                    debug!("Backtick released in dead zone or outside; quick action cancelled");
                                }
                            }

                            if let Some(w) = wheel_weak.upgrade() {
                                let _ = w.hide();
                            }
                            data.state = AppState::Idle;
                            data.current_highlighted_inner = -1;
                            data.current_highlighted_outer = -1;
                            data.is_expanded = false;
                        }
                    }
                    RawKeyEvent::CommandPalette => {
                        info!("Command Palette triggered via Ctrl + Shift + Space");
                        if let Some(m) = main_weak_for_loop.upgrade() {
                            let _ = m.show();
                            m.set_selected_nav(1);
                            m.set_format_search_filter("".into());
                            let cap_matrix = registry_clone.get_capability_matrix();
                            m.set_capability_matrix(make_capability_matrix_model(&cap_matrix, ""));
                            m.set_show_all_formats_modal(true);
                            tray::restore_and_focus_window("OrbitConvert — Fast, Powerful File Conversion");
                        }
                    }
                    RawKeyEvent::EscapeDown | RawKeyEvent::TabDown => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::WheelActive || data.state == AppState::Converting || data.is_preview {
                            debug!("Escape or Alt+Tab pressed; cancelling radial wheel");
                            if let Some(w) = wheel_weak.upgrade() {
                                let _ = w.hide();
                                w.set_is_preview(false);
                            }
                            data.state = AppState::Idle;
                            data.current_highlighted_inner = -1;
                            data.current_highlighted_outer = -1;
                            data.is_expanded = false;
                            data.is_preview = false;
                        }
                        if let Some(m) = main_weak_for_loop.upgrade() {
                            if m.get_show_all_formats_modal() {
                                m.set_show_all_formats_modal(false);
                            }
                        }
                    }
                }
            }

            // If wheel is active, track cursor movement and update highlighted sector
            let mut data = interaction_clone.lock().unwrap();
            if data.state == AppState::WheelActive {
                if let Ok(cur_pos) = get_current_cursor_pos() {
                    let total_inner = if data.wheel_kind == WheelKind::Convert {
                        if data.has_more {
                            data.inner_targets.len() + 1
                        } else {
                            data.inner_targets.len()
                        }
                    } else {
                        if data.has_more {
                            data.inner_actions.len() + 1
                        } else {
                            data.inner_actions.len()
                        }
                    };
                    let total_outer = if data.wheel_kind == WheelKind::Convert {
                        data.outer_targets.len()
                    } else {
                        data.outer_actions.len()
                    };

                    // Normalize physical cursor position against physical center by dividing by display scale factor
                    let (_offset, dist, angle_from_top) = normalize_cursor_to_logical(
                        cur_pos,
                        data.center_pos,
                        data.scale_factor,
                    );

                    // Auto-expansion check:
                    // If user moves towards or hovers over the "More" sector, expand the outer ring immediately.
                    let more_index = if data.wheel_kind == WheelKind::Convert {
                        data.inner_targets.len()
                    } else {
                        data.inner_actions.len()
                    };

                    if data.has_more && !data.is_expanded {
                        if is_more_sector_active(angle_from_top, dist, 72.5, 230.0, more_index, total_inner) {
                            data.is_expanded = true;
                            if let Some(w) = wheel_weak.upgrade() {
                                w.set_is_expanded(true);
                            }
                        }
                    }

                    let selection = calculate_two_level_selection_normalized(
                        dist,
                        angle_from_top,
                        72.5,  // inner dead zone radius (matches center hub radius)
                        152.0, // inner donut outer radius (boundary between inner & outer)
                        230.0, // outer concentric ring max radius (extended to edge for fluid selection)
                        data.is_expanded,
                        total_inner,
                        total_outer,
                    );

                    let (new_inner, new_outer) = match selection {
                        TwoLevelSelection::Inner(idx) => (idx as i32, -1),
                        TwoLevelSelection::Outer(idx) => (-1, idx as i32),
                        _ => (-1, -1),
                    };

                    if new_inner != data.current_highlighted_inner
                        || new_outer != data.current_highlighted_outer
                    {
                        data.current_highlighted_inner = new_inner;
                        data.current_highlighted_outer = new_outer;

                        if let Some(w) = wheel_weak.upgrade() {
                            w.set_highlighted_inner_index(new_inner);
                            w.set_highlighted_outer_index(new_outer);

                            if data.wheel_kind == WheelKind::Convert {
                                if new_outer >= 0 && (new_outer as usize) < data.outer_targets.len() {
                                    let target = data.outer_targets[new_outer as usize];
                                    w.set_status_text(
                                        format!("Release to convert to {}", target.display_label())
                                            .into(),
                                    );
                                } else if new_inner >= 0 {
                                    let u_idx = new_inner as usize;
                                    if u_idx < data.inner_targets.len() {
                                        let target = data.inner_targets[u_idx];
                                        w.set_status_text(
                                            format!("Release to convert to {}", target.display_label())
                                                .into(),
                                        );
                                    } else if data.has_more && u_idx == data.inner_targets.len() {
                                        w.set_status_text("More Formats (Move to outer ring)".into());
                                    }
                                } else {
                                    w.set_status_text("Move cursor to format".into());
                                }
                            } else {
                                // WheelKind::QuickActions
                                if new_outer >= 0 && (new_outer as usize) < data.outer_actions.len() {
                                    let act = data.outer_actions[new_outer as usize];
                                    w.set_status_text(
                                        format!("{} — {}", act.label(), act.subtitle()).into(),
                                    );
                                } else if new_inner >= 0 {
                                    let u_idx = new_inner as usize;
                                    if u_idx < data.inner_actions.len() {
                                        let act = data.inner_actions[u_idx];
                                        w.set_status_text(
                                            format!("{} — {}", act.label(), act.subtitle()).into(),
                                        );
                                    } else if data.has_more && u_idx == data.inner_actions.len() {
                                        w.set_status_text("More Actions (Move to outer ring)".into());
                                    }
                                } else {
                                    let hotkey_b = config_for_loop.lock().unwrap().hotkey_wheel_b.clone();
                                    let key_label = match hotkey_b.as_str() {
                                        "Hold Tab" => "Tab",
                                        "Hold Q" => "Q",
                                        "Hold X" => "X",
                                        "Hold C" => "C",
                                        _ => "`",
                                    };
                                    w.set_status_text(
                                        format!("Move cursor to action, release {} to execute", key_label).into(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        },
    );

    info!("OrbitConvert event loop running.");
    slint::run_event_loop().expect("Slint event loop error");
}

fn run_standalone_diagnostics(override_pt: Option<Point>) {
    use std::io::Write;
    println!("============================================================");
    println!("  OrbitConvert Explorer Diagnostic Utility");
    println!("============================================================");

    let pt =
        override_pt.unwrap_or_else(|| get_current_cursor_pos().unwrap_or(Point::new(0.0, 0.0)));
    println!("Target diagnostic coordinate: ({}, {})", pt.x, pt.y);

    let resolver = ExplorerResolver::new();
    let report = resolver.diagnose_cursor(pt);
    print_diagnostic_report(&report);
    let _ = std::io::stdout().flush();
}

fn print_diagnostic_report(report: &ExplorerDiagnosticReport) {
    println!("--- Explorer Diagnostic Report ---");
    println!(
        "Cursor Position: ({}, {})",
        report.cursor_point.0, report.cursor_point.1
    );
    println!("Window HWND: 0x{:X}", report.window_hwnd);
    println!("Root HWND: 0x{:X}", report.root_hwnd);
    println!("Window Class: {}", report.window_class);
    println!("Is Explorer/Desktop: {}", report.is_explorer_or_desktop);
    println!("UI Automation Element Name: {:?}", report.uia_element_name);
    println!("Clean Element Name: {:?}", report.clean_element_name);
    println!("UI Automation Control Type: {:?}", report.uia_control_type);
    println!("Shell Folder Path: {:?}", report.shell_folder_path);
    println!("Resolved Item Path: {:?}", report.resolved_item_path);
    println!("Detected Format: {:?}", report.detected_format);
    if let Some(ref reason) = report.rejection_reason {
        println!("Rejection / Status: {}", reason);
    } else {
        println!("Status: File resolved and supported for radial conversion!");
    }
    println!("----------------------------------");
}

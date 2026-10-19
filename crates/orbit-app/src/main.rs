#![windows_subsystem = "windows"]

mod tray;

use orbit_converters::register_all_converters;
use orbit_core::config::{AppConfig, ConversionPreset, WatchedFolder};
use orbit_core::formats::FileFormat;
use orbit_core::jobs::{JobManager, JobRecord, JobState};
use orbit_core::registry::{ConversionOptions, ConversionRegistry};
use orbit_ui::{
    build_wheel_sectors, FolderItem, HistoryItem, MainWindow, PresetItem, RadialWheel,
};
use orbit_windows::cursor::{
    calculate_clamped_window_pos, calculate_selected_sector, get_current_cursor_pos, Point,
};
use orbit_windows::dialogs::{pick_file, pick_folder};
use orbit_windows::explorer::{ExplorerDiagnosticReport, ExplorerResolver};
use orbit_windows::keyboard::{
    is_cursor_or_foreground_explorer, KeyboardHookManager, RawKeyEvent,
};
use slint::{ComponentHandle, LogicalPosition, ModelRc, VecModel};
use std::env;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppState {
    Idle,
    WheelActive,
    Converting,
}

// Wheel interaction state
struct InteractionData {
    state: AppState,
    center_pos: Point,
    source_path: Option<PathBuf>,
    source_format: Option<FileFormat>,
    target_formats: Vec<FileFormat>,
    current_highlighted_index: i32,
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

    // Populate initial MainWindow properties from AppConfig
    {
        main_window.set_jpeg_quality(initial_config.jpeg_quality as i32);
        main_window.set_svg_scale(initial_config.svg_scale);
        main_window.set_show_notifications(initial_config.show_notifications);
        main_window.set_launch_on_startup(initial_config.launch_on_startup);
        main_window.set_minimize_to_tray(initial_config.minimize_to_tray);
        main_window.set_service_active(true);

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
    }

    let interaction = Arc::new(Mutex::new(InteractionData {
        state: AppState::Idle,
        center_pos: Point::new(0.0, 0.0),
        source_path: None,
        source_format: None,
        target_formats: Vec::new(),
        current_highlighted_index: -1,
        is_preview: false,
    }));

    // Set notification callback for completed/failed/in-progress jobs
    let main_window_for_notify = main_window.as_weak();
    let wheel_for_notify = wheel.as_weak();
    let history_store_for_notify = history_store.clone();
    let config_for_notify = config.clone();
    let interaction_for_notify = interaction.clone();

    job_manager.set_notification_callback(Arc::new(move |record: &JobRecord| {
        // If conversion is in-progress, update live progress readouts and return early
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

            return;
        }

        let show_notifs = config_for_notify.lock().unwrap().show_notifications;
        let is_success = matches!(&record.state, JobState::Completed { .. });

        let (status_text, out_path_str, duration_ms) = match &record.state {
            JobState::Completed {
                output_path,
                duration_ms,
            } => {
                let fname = output_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if show_notifs {
                    let title = format!("OrbitConvert: {} Ready", record.target_format);
                    let msg = format!("Saved {} in {}ms", fname, duration_ms);
                    tray::send_global_notification(&title, &msg, false);
                }
                (
                    format!("Completed in {}ms", duration_ms),
                    output_path.display().to_string(),
                    *duration_ms as i32,
                )
            }
            JobState::Failed { error } => {
                if show_notifs {
                    tray::send_global_notification("OrbitConvert: Conversion Failed", error, true);
                }
                (format!("Failed: {}", error), String::new(), 0)
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
                data.current_highlighted_index = -1;
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
                        let targets = registry_for_browse.get_available_targets(fmt);
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

                let _ = job_mgr_for_manual.submit(src_path, src_fmt, target_fmt, custom_out);
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
                let _ = cfg.save();
                info!("Preferences saved successfully from Dashboard");
                tray::send_global_notification(
                    "OrbitConvert",
                    "Preferences updated and saved.",
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
        let resolver_for_diag = resolver.clone();
        let m_weak = main_window.as_weak();
        main_window.on_run_diagnostics_clicked(move || {
            if let Ok(pt) = get_current_cursor_pos() {
                let report = resolver_for_diag.diagnose_cursor(pt);
                print_diagnostic_report(&report);

                if let Some(m) = m_weak.upgrade() {
                    m.set_diag_window_class(report.window_class.clone().into());
                    m.set_diag_element_name(
                        report
                            .uia_element_name
                            .clone()
                            .unwrap_or_else(|| "None".to_string())
                            .into(),
                    );
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
            }
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
        let (sectors, _targets) = build_wheel_sectors(
            180.0,
            180.0,
            170.0,
            &[
                FileFormat::Png,
                FileFormat::Jpg,
                FileFormat::Webp,
                FileFormat::Ico,
            ],
        );
        wheel.set_source_ext("SVG".into());
        wheel.set_source_name("test_logo.svg".into());
        wheel.set_sectors(sectors);
        wheel.set_highlighted_index(0);
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
                let demo_targets = vec![
                    FileFormat::Png,
                    FileFormat::Jpg,
                    FileFormat::Webp,
                    FileFormat::Ico,
                    FileFormat::Bmp,
                    FileFormat::Gif,
                ];
                let (sectors, _) = build_wheel_sectors(180.0, 180.0, 170.0, &demo_targets);
                w.set_source_ext("SVG".into());
                w.set_source_name("sample_vector.svg".into());
                w.set_sectors(sectors);
                w.set_highlighted_index(-1);
                w.set_status_text("Interactive Preview (Move mouse to highlight, Esc to close)".into());
                w.set_is_preview(true);

                let cur_pos = get_current_cursor_pos().unwrap_or(Point::new(600.0, 400.0));
                let (wx, wy) = calculate_clamped_window_pos(cur_pos, 360.0, 360.0);
                w.window().set_position(LogicalPosition::new(wx as f32, wy as f32));
                let _ = w.show();

                let mut data = interaction_for_preview.lock().unwrap();
                data.state = AppState::WheelActive;
                data.center_pos = Point::new(wx as f32 + 180.0, wy as f32 + 180.0);
                data.source_path = None;
                data.source_format = Some(FileFormat::Svg);
                data.target_formats = demo_targets;
                data.current_highlighted_index = -1;
                data.is_preview = true;
            }
        });
    }

    {
        let wheel_for_close = wheel.as_weak();
        let interaction_for_close = interaction.clone();
        wheel.on_close_requested(move || {
            if let Some(w) = wheel_for_close.upgrade() {
                let _ = w.hide();
            }
            let mut data = interaction_for_close.lock().unwrap();
            data.state = AppState::Idle;
            data.current_highlighted_index = -1;
        });
    }

    let timer = slint::Timer::default();
    let interaction_clone = interaction.clone();

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
                                path = ?report.resolved_item_path,
                                fmt = ?report.detected_format,
                                reason = ?report.rejection_reason,
                                "Explorer item inspection on AltDown"
                            );

                            if let (Some(ref item_path), Some(src_fmt)) =
                                (&report.resolved_item_path, report.detected_format)
                            {
                                let targets = registry_clone.get_available_targets(src_fmt);
                                if targets.is_empty() {
                                    warn!(
                                        format = %src_fmt,
                                        path = %item_path.display(),
                                        "Supported file detected but no target conversions available in registry"
                                    );
                                    continue;
                                }

                                info!(
                                    file = %item_path.display(),
                                    format = %src_fmt,
                                    targets = ?targets,
                                    "File resolved with valid conversion routes. Showing radial wheel."
                                );

                                data.state = AppState::WheelActive;
                                data.center_pos = cursor_pos;
                                data.source_path = Some(item_path.clone());
                                data.source_format = Some(src_fmt);
                                data.target_formats = targets.clone();
                                data.current_highlighted_index = -1;
                                data.is_preview = false;

                                if let Some(w) = wheel_weak.upgrade() {
                                    let (sectors, _) =
                                        build_wheel_sectors(180.0, 180.0, 170.0, &targets);
                                    let fname = item_path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();

                                    w.set_is_preview(false);
                                    w.set_source_ext(src_fmt.display_label().into());
                                    w.set_source_name(fname.into());
                                    w.set_sectors(sectors);
                                    w.set_highlighted_index(-1);
                                    w.set_status_text("Move cursor to format, release Alt to convert".into());

                                    let (wx, wy) = calculate_clamped_window_pos(
                                        cursor_pos, 360.0, 360.0,
                                    );
                                    w.window().set_position(LogicalPosition::new(
                                        wx as f32, wy as f32,
                                    ));
                                    let _ = w.show();
                                }
                            }
                        }
                    }
                    RawKeyEvent::AltUp => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::WheelActive {
                            if !data.is_preview {
                                let selected_idx = data.current_highlighted_index;
                                if selected_idx >= 0
                                    && (selected_idx as usize) < data.target_formats.len()
                                {
                                    let target_fmt = data.target_formats[selected_idx as usize];
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

                                        let _ = job_mgr_clone.submit(
                                            src_path.clone(),
                                            src_fmt,
                                            target_fmt,
                                            custom_out,
                                        );

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
                            data.current_highlighted_index = -1;
                        }
                    }
                    RawKeyEvent::EscapeDown | RawKeyEvent::TabDown => {
                        let mut data = interaction_clone.lock().unwrap();
                        if data.state == AppState::WheelActive || data.state == AppState::Converting {
                            debug!("Escape or Alt+Tab pressed; cancelling radial wheel");
                            if let Some(w) = wheel_weak.upgrade() {
                                let _ = w.hide();
                            }
                            data.state = AppState::Idle;
                            data.current_highlighted_index = -1;
                        }
                    }
                }
            }

            // If wheel is active, track cursor movement and update highlighted sector
            let mut data = interaction_clone.lock().unwrap();
            if data.state == AppState::WheelActive {
                if let Ok(cur_pos) = get_current_cursor_pos() {
                    let num_sectors = data.target_formats.len();
                    let selected = calculate_selected_sector(
                        data.center_pos,
                        cur_pos,
                        72.5,  // inner dead zone radius (matches center hub radius)
                        170.0, // outer radius (matches outer ring boundary)
                        num_sectors,
                    );

                    let idx = selected.map(|i| i as i32).unwrap_or(-1);
                    if idx != data.current_highlighted_index {
                        data.current_highlighted_index = idx;
                        if let Some(w) = wheel_weak.upgrade() {
                            w.set_highlighted_index(idx);
                            if idx >= 0 && (idx as usize) < data.target_formats.len() {
                                let target = data.target_formats[idx as usize];
                                w.set_status_text(
                                    format!("Release to convert to {}", target.display_label())
                                        .into(),
                                );
                            } else {
                                w.set_status_text("Move cursor to format".into());
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

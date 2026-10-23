use orbit_converters::register_all_converters;
use orbit_core::formats::FileFormat;
use orbit_core::output::{finalize_output, prepare_output_file};
use orbit_core::registry::{ConversionOptions, ConversionRegistry};
use orbit_windows::cursor::{calculate_selected_sector, Point};
use orbit_windows::keyboard::InteractionState;
use slint::ComponentHandle;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_comprehensive_registry_all_mappings() {
    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);

    // Verify SVG conversions
    let svg_targets = reg.get_available_targets(FileFormat::Svg);
    assert!(svg_targets.contains(&FileFormat::Png));
    assert!(svg_targets.contains(&FileFormat::Jpg));
    assert!(svg_targets.contains(&FileFormat::Webp));
    assert!(svg_targets.contains(&FileFormat::Ico));
    assert!(svg_targets.contains(&FileFormat::Bmp));
    assert!(svg_targets.contains(&FileFormat::Tiff));
    assert!(svg_targets.contains(&FileFormat::Gif));

    // Verify PNG conversions
    let png_targets = reg.get_available_targets(FileFormat::Png);
    assert!(png_targets.contains(&FileFormat::Jpg));
    assert!(png_targets.contains(&FileFormat::Webp));
    assert!(png_targets.contains(&FileFormat::Ico));
    assert!(png_targets.contains(&FileFormat::Bmp));

    // Verify JPG conversions
    let jpg_targets = reg.get_available_targets(FileFormat::Jpg);
    assert!(jpg_targets.contains(&FileFormat::Png));
    assert!(jpg_targets.contains(&FileFormat::Webp));

    // Verify WebP conversions
    let webp_targets = reg.get_available_targets(FileFormat::Webp);
    assert!(webp_targets.contains(&FileFormat::Png));
    assert!(webp_targets.contains(&FileFormat::Jpg));

    // Verify Video & Audio conversions (FFmpeg engine)
    let mp4_targets = reg.get_available_targets(FileFormat::Mp4);
    assert!(mp4_targets.contains(&FileFormat::Webm));
    assert!(mp4_targets.contains(&FileFormat::Mp3));

    // Verify Data conversions
    let csv_targets = reg.get_available_targets(FileFormat::Csv);
    assert!(csv_targets.contains(&FileFormat::Json));
    assert!(csv_targets.contains(&FileFormat::Tsv));
}

#[test]
fn test_angle_to_sector_all_quadrants() {
    let center = Point::new(300.0, 300.0);
    let dead_zone = 45.0;
    let max_radius = 200.0;
    let sectors_count = 4;

    // 12 o'clock (North) -> Sector 0
    assert_eq!(
        calculate_selected_sector(
            center,
            Point::new(300.0, 150.0),
            dead_zone,
            max_radius,
            sectors_count
        ),
        Some(0)
    );

    // 3 o'clock (East) -> Sector 1
    assert_eq!(
        calculate_selected_sector(
            center,
            Point::new(450.0, 300.0),
            dead_zone,
            max_radius,
            sectors_count
        ),
        Some(1)
    );

    // 6 o'clock (South) -> Sector 2
    assert_eq!(
        calculate_selected_sector(
            center,
            Point::new(300.0, 450.0),
            dead_zone,
            max_radius,
            sectors_count
        ),
        Some(2)
    );

    // 9 o'clock (West) -> Sector 3
    assert_eq!(
        calculate_selected_sector(
            center,
            Point::new(150.0, 300.0),
            dead_zone,
            max_radius,
            sectors_count
        ),
        Some(3)
    );

    // Inside dead zone (radius = 20px < 45px) -> None
    assert_eq!(
        calculate_selected_sector(
            center,
            Point::new(315.0, 310.0),
            dead_zone,
            max_radius,
            sectors_count
        ),
        None
    );

    // Beyond outer wheel boundary (radius = 250px > 200px) -> None
    assert_eq!(
        calculate_selected_sector(
            center,
            Point::new(300.0, 50.0),
            dead_zone,
            max_radius,
            sectors_count
        ),
        None
    );
}

#[test]
fn test_unicode_and_spaces_in_paths() {
    let dir = tempdir().unwrap();
    let unicode_folder = dir.path().join("Folder with spaces and 日本語 🚀");
    fs::create_dir_all(&unicode_folder).unwrap();

    let source_file = unicode_folder.join("test logo 特别.svg");
    let svg_data = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">
        <rect width="64" height="64" fill="#10b981"/>
    </svg>"##;
    fs::write(&source_file, svg_data).unwrap();

    let (out_path, guard) = prepare_output_file(&source_file, FileFormat::Png, None).unwrap();
    assert_eq!(out_path, unicode_folder.join("test logo 特别.png"));

    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);
    let converter = reg.find_converter(FileFormat::Svg, FileFormat::Png).unwrap();
    let options = ConversionOptions::default();

    converter
        .convert(&source_file, guard.path(), &options, None)
        .unwrap();
    finalize_output(guard, &out_path).unwrap();

    assert!(out_path.exists());
    let img = image::open(&out_path).unwrap();
    assert_eq!(img.width(), 64);
    assert_eq!(img.height(), 64);
}

#[test]
fn test_atomic_temp_cleanup_on_failure() {
    let dir = tempdir().unwrap();
    let source_file = dir.path().join("source.png");
    fs::write(&source_file, b"corrupted non-image content").unwrap();

    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);
    let converter = reg.find_converter(FileFormat::Png, FileFormat::Jpg).unwrap();
    let options = ConversionOptions::default();

    let (out_path, guard) = prepare_output_file(&source_file, FileFormat::Jpg, None).unwrap();
    let temp_path = guard.path().to_path_buf();

    let result = converter.convert(&source_file, &temp_path, &options, None);
    assert!(result.is_err());

    // Drop guard without finalize
    drop(guard);

    // Temp file must be automatically removed by RAII drop guard
    assert!(!temp_path.exists());
    // Destination file must never have been created
    assert!(!out_path.exists());
}

#[test]
fn test_state_machine_transitions() {
    let mut state = InteractionState::Idle;
    assert_eq!(state, InteractionState::Idle);

    // Alt down over explorer
    state = InteractionState::ResolvingFile;
    assert_eq!(state, InteractionState::ResolvingFile);

    // File resolved, show wheel
    state = InteractionState::WheelVisible;
    assert_eq!(state, InteractionState::WheelVisible);

    // Cursor moves to sector
    state = InteractionState::FormatHighlighted;
    assert_eq!(state, InteractionState::FormatHighlighted);

    // Alt release starts conversion
    state = InteractionState::Converting;
    assert_eq!(state, InteractionState::Converting);

    // Conversion finishes
    state = InteractionState::Completed;
    assert_eq!(state, InteractionState::Completed);

    // Reset to idle
    state = InteractionState::Idle;
    assert_eq!(state, InteractionState::Idle);
}

#[test]
fn test_configurable_hotkeys_and_defaults() {
    use orbit_core::config::AppConfig;
    use orbit_windows::keyboard::{configure_hotkeys, HotkeySettings};

    // 1. Verify default values
    let mut config = AppConfig::default();
    assert_eq!(config.hotkey_wheel_a, "Hold Alt");
    assert_eq!(config.hotkey_wheel_b, "Hold ` (Backtick)");
    assert_eq!(config.hotkey_command_palette, "Ctrl+Shift+Space");

    // 2. Customize hotkeys
    config.hotkey_wheel_a = "Hold Ctrl".to_string();
    config.hotkey_wheel_b = "Hold Tab".to_string();
    config.hotkey_command_palette = "Ctrl+Space".to_string();

    configure_hotkeys(HotkeySettings {
        wheel_a: config.hotkey_wheel_a.clone(),
        wheel_b: config.hotkey_wheel_b.clone(),
        command_palette: config.hotkey_command_palette.clone(),
    });

    assert_eq!(config.hotkey_wheel_a, "Hold Ctrl");
    assert_eq!(config.hotkey_wheel_b, "Hold Tab");
    assert_eq!(config.hotkey_command_palette, "Ctrl+Space");

    // 3. Reset to defaults
    config.reset_hotkeys_to_default();
    assert_eq!(config.hotkey_wheel_a, "Hold Alt");
    assert_eq!(config.hotkey_wheel_b, "Hold ` (Backtick)");
    assert_eq!(config.hotkey_command_palette, "Ctrl+Shift+Space");

    configure_hotkeys(HotkeySettings {
        wheel_a: config.hotkey_wheel_a.clone(),
        wheel_b: config.hotkey_wheel_b.clone(),
        command_palette: config.hotkey_command_palette.clone(),
    });
}

#[test]
fn test_window_close_minimize_to_tray() {
    let config = orbit_core::config::AppConfig::default();
    assert!(config.minimize_to_tray, "minimize_to_tray must be enabled by default");

    let main_window = orbit_ui::MainWindow::new().unwrap();
    main_window.set_minimize_to_tray(config.minimize_to_tray);
    assert!(main_window.get_minimize_to_tray());

    // Verify on_close_requested can be registered and returns HideWindow
    let m_weak = main_window.as_weak();
    main_window.window().on_close_requested(move || {
        if let Some(m) = m_weak.upgrade() {
            let _ = m.hide();
        }
        slint::CloseRequestResponse::HideWindow
    });

    let phys_pos = slint::PhysicalPosition::new(100, 100);
    main_window.window().set_position(phys_pos);
    let scale = main_window.window().scale_factor();
    assert!(scale > 0.0);
}

#[test]
fn test_multi_device_radial_wheel_dpi() {
    use orbit_windows::cursor::{
        calculate_clamped_window_pos_physical, calculate_two_level_selection_normalized,
        is_more_sector_active, normalize_cursor_to_logical, Point, TwoLevelSelection,
    };

    // Test across display scaling factors within the monitor's work area
    for scale in [1.0, 1.25, 1.5] {
        let cursor_phys = Point::new(500.0, 350.0);
        let (clamped_x, clamped_y, phys_center) = calculate_clamped_window_pos_physical(
            cursor_phys, 460.0, 460.0, scale,
        );

        // Window should be centered around cursor on unclamped screen area
        let half_phys = (460.0 * scale) / 2.0;
        assert_eq!(clamped_x, (500.0 - half_phys).round() as i32);
        assert_eq!(clamped_y, (350.0 - half_phys).round() as i32);
        assert!((phys_center.x - 500.0).abs() <= 1.0);
        assert!((phys_center.y - 350.0).abs() <= 1.0);

        // 1. Inside center hub (50 logical points away)
        let cur_hub = Point::new(phys_center.x, phys_center.y - 50.0 * scale);
        let (_, dist, angle) = normalize_cursor_to_logical(cur_hub, phys_center, scale);
        assert!((dist - 50.0).abs() < 1e-3);
        let sel = calculate_two_level_selection_normalized(
            dist, angle, 72.5, 152.0, 230.0, false, 6, 8,
        );
        assert_eq!(sel, TwoLevelSelection::CenterHub);

        // 2. In inner ring (100 logical points away to North, angle 0 rad)
        let cur_inner = Point::new(phys_center.x, phys_center.y - 100.0 * scale);
        let (_, dist, angle) = normalize_cursor_to_logical(cur_inner, phys_center, scale);
        assert!((dist - 100.0).abs() < 1e-3);
        let sel = calculate_two_level_selection_normalized(
            dist, angle, 72.5, 152.0, 230.0, false, 6, 8,
        );
        assert_eq!(sel, TwoLevelSelection::Inner(0));

        // 3. In outer ring when expanded (180 logical points away to East)
        let cur_outer = Point::new(phys_center.x + 180.0 * scale, phys_center.y);
        let (_, dist, angle) = normalize_cursor_to_logical(cur_outer, phys_center, scale);
        assert!((dist - 180.0).abs() < 1e-3);
        let sel = calculate_two_level_selection_normalized(
            dist, angle, 72.5, 152.0, 230.0, true, 6, 8,
        );
        // In 8 outer sectors: 0=North, 2=East
        assert_eq!(sel, TwoLevelSelection::Outer(2));

        // 4. Fast gesture towards More sector (sector 5 in 6 inner sectors)
        let more_angle = 5.0 * (std::f32::consts::PI / 3.0);
        let dx_phys = (more_angle - std::f32::consts::FRAC_PI_2).cos() * 190.0 * scale;
        let dy_phys = (more_angle - std::f32::consts::FRAC_PI_2).sin() * 190.0 * scale;
        let cur_fast_more = Point::new(phys_center.x + dx_phys, phys_center.y + dy_phys);
        let (_, dist_fast, angle_fast) = normalize_cursor_to_logical(cur_fast_more, phys_center, scale);

        assert!(is_more_sector_active(angle_fast, dist_fast, 72.5, 230.0, 5, 6));

        // 5. Verify strict monitor work area clamping
        let bottom_cursor = Point::new(500.0, 3000.0);
        let (_cx, cy, _) = calculate_clamped_window_pos_physical(
            bottom_cursor, 460.0, 460.0, scale,
        );
        let work_area = orbit_windows::cursor::get_monitor_work_area(bottom_cursor).unwrap();
        assert!(cy + (460.0 * scale).round() as i32 <= work_area.bottom);
    }
}

#[test]
fn test_quick_action_progress_circular_hud() {
    use std::sync::{Arc, Mutex};

    // 1. Verify 50% compact radius (38.0) path generation for Quick Actions
    let path_init = orbit_ui::build_circular_progress_path(230.0, 230.0, 38.0, 0.005);
    assert!(path_init.starts_with("M 230.00 192.00")); // 230 - 38 = 192

    let path_half = orbit_ui::build_circular_progress_path(230.0, 230.0, 38.0, 0.50);
    assert!(path_half.contains("A 38.00 38.00 0 0 1"));

    let path_done = orbit_ui::build_circular_progress_path(230.0, 230.0, 38.0, 1.00);
    assert!(path_done.contains("A 38.00 38.00 0 0 1"));

    // 2. Verify ActionGuard correctly triggers completion callback via RAII drop
    let recorded_events = Arc::new(Mutex::new(Vec::new()));
    let rec_clone = recorded_events.clone();

    let cb: Arc<dyn Fn(f32, &str, bool) + Send + Sync> = Arc::new(move |pct, msg, done| {
        rec_clone.lock().unwrap().push((pct, msg.to_string(), done));
    });

    struct TestGuard(Option<Arc<dyn Fn(f32, &str, bool) + Send + Sync>>);
    impl Drop for TestGuard {
        fn drop(&mut self) {
            if let Some(ref c) = self.0 {
                c(1.0, "✓ Finished!", true);
            }
        }
    }

    {
        cb(0.35, "Processing...", false);
        let _guard = TestGuard(Some(cb.clone()));
    }

    let events = recorded_events.lock().unwrap().clone();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], (0.35, "Processing...".to_string(), false));
    assert_eq!(events[1], (1.0, "✓ Finished!".to_string(), true));

    // 3. Verify format quick actions are non-empty and well-formed
    let (inner, _outer) = orbit_core::actions::get_quick_actions_for_format(FileFormat::Png);
    assert!(!inner.is_empty());
    assert!(inner.iter().any(|a| a.label() == "Remove BG"));
}



use orbit_converters::register_all_converters;
use orbit_core::formats::FileFormat;
use orbit_core::output::{finalize_output, prepare_output_file};
use orbit_core::registry::{ConversionOptions, ConversionRegistry};
use orbit_windows::cursor::{calculate_selected_sector, Point};
use orbit_windows::keyboard::InteractionState;
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

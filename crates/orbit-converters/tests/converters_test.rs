use orbit_converters::register_all_converters;
use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionOptions, ConversionRegistry};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_registry_integration() {
    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);

    // SVG can convert to PNG, JPG, WebP, ICO
    let svg_targets = reg.get_available_targets(FileFormat::Svg);
    assert!(svg_targets.contains(&FileFormat::Png));
    assert!(svg_targets.contains(&FileFormat::Jpg));
    assert!(svg_targets.contains(&FileFormat::Webp));
    assert!(svg_targets.contains(&FileFormat::Ico));

    // PNG can convert to JPG, WebP, ICO, BMP, TIFF
    let png_targets = reg.get_available_targets(FileFormat::Png);
    assert!(png_targets.contains(&FileFormat::Jpg));
    assert!(png_targets.contains(&FileFormat::Webp));
    assert!(png_targets.contains(&FileFormat::Ico));

    // JPG can convert to PNG, WebP
    let jpg_targets = reg.get_available_targets(FileFormat::Jpg);
    assert!(jpg_targets.contains(&FileFormat::Png));
    assert!(jpg_targets.contains(&FileFormat::Webp));
}

#[test]
fn test_svg_to_png_and_jpg_and_webp() {
    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);

    let dir = tempdir().unwrap();
    let svg_path = dir.path().join("test.svg");
    let svg_content = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
        <rect width="100" height="100" fill="#3b82f6"/>
        <circle cx="50" cy="50" r="40" fill="#ffffff" />
    </svg>"##;
    fs::write(&svg_path, svg_content).unwrap();

    let options = ConversionOptions::default();

    // 1. SVG -> PNG
    let png_path = dir.path().join("test.png");
    let converter = reg.find_converter(FileFormat::Svg, FileFormat::Png).unwrap();
    converter.convert(&svg_path, &png_path, &options, None).unwrap();
    assert!(png_path.exists());
    let img = image::open(&png_path).unwrap();
    assert_eq!(img.width(), 100);
    assert_eq!(img.height(), 100);

    // 2. SVG -> JPG
    let jpg_path = dir.path().join("test.jpg");
    let converter = reg.find_converter(FileFormat::Svg, FileFormat::Jpg).unwrap();
    converter.convert(&svg_path, &jpg_path, &options, None).unwrap();
    assert!(jpg_path.exists());
    let img_jpg = image::open(&jpg_path).unwrap();
    assert_eq!(img_jpg.width(), 100);

    // 3. SVG -> WebP
    let webp_path = dir.path().join("test.webp");
    let converter = reg.find_converter(FileFormat::Svg, FileFormat::Webp).unwrap();
    converter.convert(&svg_path, &webp_path, &options, None).unwrap();
    assert!(webp_path.exists());
    let img_webp = image::open(&webp_path).unwrap();
    assert_eq!(img_webp.width(), 100);
}

#[test]
fn test_png_to_jpg_with_transparency_compositing() {
    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);

    let dir = tempdir().unwrap();
    let png_path = dir.path().join("alpha.png");

    // Create a 10x10 transparent PNG with a red pixel
    let mut rgba_img = image::RgbaImage::new(10, 10);
    // Background is transparent (0, 0, 0, 0)
    // Pixel (5, 5) is semi-transparent red (255, 0, 0, 128)
    rgba_img.put_pixel(5, 5, image::Rgba([255, 0, 0, 128]));
    rgba_img.save(&png_path).unwrap();

    let jpg_path = dir.path().join("alpha.jpg");
    let converter = reg.find_converter(FileFormat::Png, FileFormat::Jpg).unwrap();
    let options = ConversionOptions {
        background_color: (255, 255, 255), // white background
        ..Default::default()
    };
    converter.convert(&png_path, &jpg_path, &options, None).unwrap();
    assert!(jpg_path.exists());

    let decoded_jpg = image::open(&jpg_path).unwrap().to_rgb8();
    // Transparent pixel (0, 0) should be composited to white (255, 255, 255)
    let bg_pixel = decoded_jpg.get_pixel(0, 0);
    assert!(bg_pixel[0] >= 250 && bg_pixel[1] >= 250 && bg_pixel[2] >= 250);
}

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

#[test]
fn test_model3d_conversions() {
    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);

    let dir = tempdir().unwrap();
    let obj_path = dir.path().join("cube.obj");
    let obj_content = r#"# Simple Pyramid
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.5 1.0 0.0
v 0.5 0.5 1.0
f 1 2 3
f 1 2 4
f 2 3 4
f 3 1 4
"#;
    fs::write(&obj_path, obj_content).unwrap();
    let options = ConversionOptions::default();

    // 1. OBJ -> STL
    let stl_path = dir.path().join("pyramid.stl");
    let converter = reg.find_converter(FileFormat::Obj, FileFormat::Stl).unwrap();
    converter.convert(&obj_path, &stl_path, &options, None).unwrap();
    assert!(stl_path.exists());
    assert!(fs::metadata(&stl_path).unwrap().len() > 84);

    // 2. STL -> PLY
    let ply_path = dir.path().join("pyramid.ply");
    let converter = reg.find_converter(FileFormat::Stl, FileFormat::Ply).unwrap();
    converter.convert(&stl_path, &ply_path, &options, None).unwrap();
    assert!(ply_path.exists());
    let ply_content = fs::read_to_string(&ply_path).unwrap();
    assert!(ply_content.contains("ply"));
    assert!(ply_content.contains("element vertex"));

    // 3. OBJ -> GLB
    let glb_path = dir.path().join("pyramid.glb");
    let converter = reg.find_converter(FileFormat::Obj, FileFormat::Glb).unwrap();
    converter.convert(&obj_path, &glb_path, &options, None).unwrap();
    assert!(glb_path.exists());
    let glb_bytes = fs::read(&glb_path).unwrap();
    // Verify glTF magic header 'glTF' (0x46546C67)
    assert_eq!(&glb_bytes[0..4], b"glTF");
}

#[test]
fn test_document_conversions() {
    let mut reg = ConversionRegistry::new();
    register_all_converters(&mut reg);

    let dir = tempdir().unwrap();
    let md_path = dir.path().join("readme.md");
    let md_content = "# Project Title\n\nThis is a test paragraph.\n\n- Feature 1\n- Feature 2\n";
    fs::write(&md_path, md_content).unwrap();
    let options = ConversionOptions::default();

    // 1. Markdown -> HTML
    let html_path = dir.path().join("readme.html");
    let converter = reg.find_converter(FileFormat::Md, FileFormat::Html).unwrap();
    converter.convert(&md_path, &html_path, &options, None).unwrap();
    assert!(html_path.exists());
    let html_content = fs::read_to_string(&html_path).unwrap();
    assert!(html_content.contains("<h1>Project Title</h1>"));
    assert!(html_content.contains("<li>Feature 1</li>"));

    // 2. HTML -> TXT
    let txt_path = dir.path().join("readme.txt");
    let converter = reg.find_converter(FileFormat::Html, FileFormat::Txt).unwrap();
    converter.convert(&html_path, &txt_path, &options, None).unwrap();
    assert!(txt_path.exists());
    let txt_content = fs::read_to_string(&txt_path).unwrap();
    assert!(txt_content.contains("Project Title"));
    assert!(txt_content.contains("Feature 1"));
}

#[test]
fn test_background_removal() {
    use image::{Rgba, RgbaImage};
    use orbit_converters::bg_removal::pure_rust_remove_background;

    let dir = tempdir().unwrap();
    let in_path = dir.path().join("test_subject.png");
    let out_path = dir.path().join("test_subject_nobg.png");

    // Create 100x100 white image with red circle at center
    let mut img = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
    for y in 30..70 {
        for x in 30..70 {
            let dx = x as i32 - 50;
            let dy = y as i32 - 50;
            if dx * dx + dy * dy <= 18 * 18 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
    }
    img.save(&in_path).unwrap();

    // Run background removal
    pure_rust_remove_background(&in_path, &out_path).unwrap();
    assert!(out_path.exists());

    let res = image::open(&out_path).unwrap().to_rgba8();
    // Verify corner is transparent
    let corner = res.get_pixel(0, 0);
    assert_eq!(corner[3], 0, "Corner pixel should be fully transparent");

    // Verify center red circle is opaque
    let center = res.get_pixel(50, 50);
    assert_eq!(center[0], 255);
    assert_eq!(center[1], 0);
    assert_eq!(center[2], 0);
    assert_eq!(center[3], 255, "Center subject should remain fully opaque");
}

#[test]
fn test_python_worker_integration() {
    use orbit_converters::python_worker::get_python_worker;

    let worker = get_python_worker();
    if !worker.is_available() {
        println!("Python worker not available in test environment, skipping");
        return;
    }

    // 1. Capabilities
    let caps = worker.get_capabilities().unwrap();
    assert_eq!(caps["engine"], "OrbitEngine-Python");
    assert!(caps["quick_actions"]["remove_background"]["available"].as_bool().unwrap_or(false));

    // 2. Dependency status
    let deps = worker.get_dependency_status().unwrap();
    assert!(deps["python_packages"]["rembg"]["installed"].as_bool().unwrap_or(false));

    // 3. AI background removal on test image
    let dir = tempdir().unwrap();
    let in_path = dir.path().join("ai_test_in.png");
    let out_path = dir.path().join("ai_test_out.png");

    let mut img = image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]));
    for y in 16..48 {
        for x in 16..48 {
            img.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
        }
    }
    img.save(&in_path).unwrap();

    let saved = orbit_converters::bg_removal::remove_background(&in_path, &out_path).unwrap();
    assert!(saved.exists());
    let res = image::open(&saved).unwrap().to_rgba8();
    assert_eq!(res.width(), 64);
    assert_eq!(res.height(), 64);
}



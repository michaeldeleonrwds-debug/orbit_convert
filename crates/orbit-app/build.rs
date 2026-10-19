fn main() {
    #[cfg(windows)]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let ico_path = std::path::Path::new(&manifest_dir).join("../../assets/app_icon.ico");
        
        if ico_path.exists() {
            let rc_path = std::path::Path::new(&out_dir).join("app_icon.rc");
            let res_path = std::path::Path::new(&out_dir).join("app_icon.o");
            let ico_str = ico_path.to_string_lossy().replace('\\', "/");
            let rc_content = format!("1 ICON \"{}\"\n", ico_str);
            if std::fs::write(&rc_path, rc_content).is_ok() {
                let windres = std::process::Command::new("windres")
                    .args(&["-i", rc_path.to_str().unwrap(), "-o", res_path.to_str().unwrap()])
                    .status();

                if let Ok(status) = windres {
                    if status.success() {
                        println!("cargo:rustc-link-arg={}", res_path.display());
                    }
                }
            }
        }
    }
}

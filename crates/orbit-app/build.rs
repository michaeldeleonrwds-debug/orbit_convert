fn main() {
    #[cfg(windows)]
    {
        let out_dir = std::env::var("OUT_DIR").unwrap_or_default();
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let ico_path = std::path::Path::new(&manifest_dir).join("../../assets/app_icon.ico");
        
        let canonical_ico = ico_path.canonicalize().unwrap_or(ico_path.clone());
        if canonical_ico.exists() {
            let rc_path = std::path::Path::new(&out_dir).join("app_icon.rc");
            let res_path = std::path::Path::new(&out_dir).join("app_icon.o");
            let raw_str = canonical_ico.to_string_lossy();
            let ico_str = raw_str.trim_start_matches(r"\\?\").replace('\\', "/");
            let rc_content = format!("1 ICON \"{}\"\n", ico_str);
            if std::fs::write(&rc_path, rc_content).is_ok() {
                let windres_cmd = if std::process::Command::new("windres").arg("--version").output().is_ok() {
                    "windres".to_string()
                } else {
                    let fallback = r"C:\Users\REYMARK\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin\windres.exe";
                    if std::path::Path::new(fallback).exists() {
                        fallback.to_string()
                    } else {
                        "windres".to_string()
                    }
                };

                let status = std::process::Command::new(&windres_cmd)
                    .args(&["-i", rc_path.to_str().unwrap(), "-o", res_path.to_str().unwrap()])
                    .status();

                if let Ok(st) = status {
                    if st.success() {
                        println!("cargo:rustc-link-arg={}", res_path.display());
                        println!("cargo:warning=Successfully compiled and linked Windows icon resource into executable");
                    } else {
                        println!("cargo:warning=windres failed with exit code: {:?}", st);
                    }
                } else if let Err(e) = status {
                    println!("cargo:warning=Failed to spawn windres: {}", e);
                }
            }
        }
    }
}

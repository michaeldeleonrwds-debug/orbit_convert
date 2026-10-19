# OrbitConvert

**OrbitConvert** is a native Windows file conversion power utility featuring an ergonomic radial conversion wheel triggered directly from Windows File Explorer.

Hover your cursor over a supported file in File Explorer, hold the **Alt** key, flick the cursor towards your desired output format, and release Alt to convert. The converted file is saved alongside the original without modifying or overwriting your source files.

---

## Architecture Overview

OrbitConvert is built as a modular Rust workspace:

```text
Converter/
├── Cargo.toml                  # Workspace configuration and dependencies
├── README.md                   # Build and architecture documentation
├── sample.svg                  # Test SVG asset
├── crates/
│   ├── orbit-core/             # Core formats, registry, output manager, and job queue
│   │   ├── src/
│   │   │   ├── formats.rs      # File format enums, MIME types, extensions, transparency traits
│   │   │   ├── registry.rs     # Converter trait, options, and conversion registry
│   │   │   ├── jobs.rs         # Async background job queue, status tracking, notifications
│   │   │   ├── output.rs       # Collision-safe naming ("file (1).ext"), atomic swap, RAII cleanup
│   │   │   ├── config.rs       # Persistent TOML configuration in %APPDATA%
│   │   │   └── lib.rs
│   │
│   ├── orbit-converters/       # Image and SVG conversion adapters
│   │   ├── src/
│   │   │   ├── images.rs       # Raster conversions: PNG, JPG, WebP, ICO, BMP, TIFF (image crate)
│   │   │   │                   # Handles transparency compositing onto solid backgrounds for JPG
│   │   │   ├── svg.rs          # High-fidelity SVG rasterization via resvg / usvg / tiny-skia
│   │   │   └── lib.rs
│   │   └── tests/
│   │       └── converters_test.rs
│   │
│   ├── orbit-ui/               # Slint native UI integration
│   │   ├── build.rs            # Slint template compiler
│   │   ├── ui/
│   │   │   ├── wheel.slint     # Minimalist, dark charcoal radial wheel with dynamic sectors
│   │   │   ├── settings.slint  # Settings dialog for shortcut, quality, scale, notifications
│   │   │   └── main.slint
│   │   └── src/
│   │       └── lib.rs          # Radial sector positioning and trigonometry
│   │
│   ├── orbit-windows/          # Windows native OS integrations
│   │   ├── src/
│   │   │   ├── keyboard.rs     # Non-blocking WH_KEYBOARD_LL low-level hook & state machine
│   │   │   ├── cursor.rs       # Cursor tracking, DPI/monitor bounds clamping, angle calculation
│   │   │   ├── automation.rs   # Windows UI Automation client for cursor hit-testing
│   │   │   ├── explorer.rs     # Shell COM (IShellWindows, IFolderView2, IShellItem) path resolution
│   │   │   ├── overlay.rs      # Native Win32 window styles (WS_EX_LAYERED, WS_EX_TOPMOST, NOACTIVATE)
│   │   │   └── lib.rs
│   │
│   └── orbit-app/              # Main desktop application and system tray
│       ├── src/
│       │   ├── tray.rs         # Native Win32 Shell_NotifyIconW, tray menu, balloon notifications
│       │   └── main.rs         # Application lifecycle, 60fps tick, CLI flags (--diagnose, --test-wheel)
│       └── tests/
│           └── workspace_tests.rs
```

---

## Key Features

### 1. Real Windows Explorer File Detection
- **UI Automation (`IUIAutomation`)**: Hit-tests the exact UI element directly underneath the cursor (in Details, List, or Icon views).
- **Shell COM (`IShellWindows` & `IFolderView2`)**: Queries the host Explorer window or Desktop, retrieves the active folder's `IShellItem`, and resolves the true absolute filesystem path with extensions (even when Windows "Hide extensions for known file types" is active).
- **Safe Filtering**: Rejects directory items, empty window areas, navigation sidebars, and unsupported shell objects.

### 2. Alt Key State Machine
- **Non-blocking Hook**: Uses a lightweight `WH_KEYBOARD_LL` hook running purely on event messages via channels; no heavy COM or disk I/O ever blocks keyboard input.
- **Strict State Machine**: `Idle` -> `ResolvingFile` -> `WheelVisible` -> `FormatHighlighted` -> `Converting` -> `Completed` / `Cancelled`.
- **Cancellation**: Pressing `Escape`, triggering `Alt+Tab`, releasing in the center dead zone, or moving focus outside Explorer instantly cancels without conversion.

### 3. Radial Wheel UI (Slint)
- **Minimalist Aesthetic**: Dark charcoal background (`#121316f0`), crisp neutral borders (`#272a30`), restrained blue highlight accent (`#3b82f6`), and white typography.
- **Center Hub**: Displays source format badge, filename, and contextual action hints ("Drag cursor to format" / "Release to convert to PNG").
- **Smart Hit Testing**: Geometry-based selection using angle and radius relative to the initial cursor position with an inner neutral dead zone.
- **Monitor-Aware Positioning**: Automatically clamps window coordinates to the monitor work area so the wheel is never clipped off-screen on multi-monitor setups.

### 4. Robust Conversion Engine
- **Supported Formats**:
  - `SVG` -> `PNG`, `JPG`, `WebP`, `ICO`
  - `PNG` -> `JPG`, `WebP`, `ICO`, `BMP`, `TIFF`
  - `JPG` -> `PNG`, `WebP`, `BMP`, `TIFF`
  - `WebP` -> `PNG`, `JPG`, `ICO`, `BMP`, `TIFF`
- **Transparency Preservation**: PNG, WebP, and ICO preserve full alpha channels. For JPG (which lacks alpha), transparent pixels are cleanly composited over a configurable background color (default white `[255, 255, 255]`).
- **Aspect Ratio & Quality**: Vector rasterization maintains aspect ratio and scales crisply with configurable scale factors and JPEG/WebP quality controls.

### 5. Atomic Output Safety
- **Collision Avoidance**: If `logo.png` already exists, OrbitConvert generates `logo (1).png`, `logo (2).png`, etc.
- **Source Protection**: Never modifies or overwrites the source file.
- **Atomic Renaming**: Writes output to a temporary `.orbit_<stem>_<timestamp>.ext` file and atomically moves it into place only after conversion succeeds.
- **RAII Drop Cleanup**: Automatically purges incomplete or corrupt temporary files if an error or cancellation occurs.

### 6. Background Processing & System Tray
- **Asynchronous Execution**: Conversions execute on a dedicated background worker thread pool.
- **Native Tray Icon**: Minimal background presence with tray menu options:
  - Toggle Active / Paused state
  - "Diagnose Hovered Item in Explorer"
  - "Settings..." configuration dialog
  - "Open Output Directory"
  - "Exit OrbitConvert"
- **Native Notifications**: Displays desktop balloon / toast notifications when conversions complete or fail.

---

## Build Instructions (Windows)

### Prerequisites

1. **Rust Toolchain**:
   Install Rust via `rustup` (Rust 1.80+ or latest stable).
   ```powershell
   # Install rustup
   winget install Rustlang.Rustup
   ```

2. **C/C++ Compiler & Linker**:
   OrbitConvert can be compiled using either:
   - **MinGW-w64 / WinLibs (Recommended for standalone/portable builds)**:
     ```powershell
     winget install BrechtSanders.WinLibs.POSIX.UCRT
     # Add MinGW bin folder to PATH (contains gcc, dlltool, ld, etc.)
     $env:PATH = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:PATH"
     rustup default stable-x86_64-pc-windows-gnu
     ```
   - **Microsoft Visual C++ (MSVC)**:
     Install Visual Studio 2022 Build Tools with the "Desktop development with C++" workload:
     ```powershell
     rustup default stable-x86_64-pc-windows-msvc
     ```

### Compiling

From the workspace root directory (`c:\Users\REYMARK\Documents\App\Converter`):

```powershell
# Development build
cargo build --workspace

# Release build (optimized)
cargo build --release --workspace
```

The resulting executable will be located at:
- `target\release\orbit-app.exe` (Release)
- `target\debug\orbit-app.exe` (Debug)

---

## Running the Application

### 1. Standard Background Tray Mode
```powershell
.\target\release\orbit-app.exe
```
Starts OrbitConvert in the system tray. Open Windows File Explorer, hover over any supported file (`.svg`, `.png`, `.jpg`, `.webp`), and hold `Alt`.

### 2. Standalone Diagnostic Mode (`--diagnose`)
Tests the Explorer detection pipeline and prints the exact window, UI Automation element, Shell folder path, and resolved filesystem path:
```powershell
.\target\release\orbit-app.exe --diagnose
```
To test specific coordinates:
```powershell
.\target\release\orbit-app.exe --diagnose-point 500 500
```

### 3. Visual Wheel Demonstration (`--test-wheel`)
Renders the radial wheel centered on the screen for 4 seconds for UX and rendering validation without needing to hold keys:
```powershell
.\target\release\orbit-app.exe --test-wheel
```

---

## Automated Test Suite

Run all automated unit and integration tests across the workspace:

```powershell
cargo test
```

### Test Coverage Summary:
- `test_comprehensive_registry_all_mappings`: Verifies all source-to-target format pairings.
- `test_angle_to_sector_all_quadrants`: Verifies 12, 3, 6, and 9 o'clock angle calculations, dead zone isolation, and outer boundary cutoffs.
- `test_unicode_and_spaces_in_paths`: Tests conversions in paths containing spaces, Unicode characters, and emoji (`Folder with spaces and 日本語 🚀`).
- `test_atomic_temp_cleanup_on_failure`: Validates RAII cleanup of temporary files upon corruption or failure.
- `test_state_machine_transitions`: Verifies interaction state machine transitions.
- `test_svg_to_png_and_jpg_and_webp`: Full rasterization tests with pixel dimension validation.
- `test_png_to_jpg_with_transparency_compositing`: Verifies alpha channel compositing onto configurable solid backgrounds.
- `test_collision_safe_naming`: Verifies generation of `file (1).ext`, `file (2).ext` without overwriting existing files.
- `test_never_overwrite_source`: Guarantees source files can never be overwritten under any condition.

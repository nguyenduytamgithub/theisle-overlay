use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path).expect("read night vision source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn win32_symbols(source: &str) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    let mut remaining = source;
    const PREFIX: &str = "use windows::Win32::";
    while let Some(start) = remaining.find(PREFIX) {
        let statement = &remaining[start..];
        let end = statement
            .find(';')
            .expect("every Windows import must be a complete use statement");
        for token in statement[..=end]
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        {
            if !token.is_empty() && !matches!(token, "use" | "windows" | "Win32") {
                symbols.insert(token.to_string());
            }
        }
        remaining = &statement[end + 1..];
    }
    symbols
}

fn has_unapproved_windows_boundary(source: &str) -> bool {
    let mut stripped = String::with_capacity(source.len());
    let mut remaining = source;
    const PREFIX: &str = "use windows::Win32::";
    while let Some(start) = remaining.find(PREFIX) {
        stripped.push_str(&remaining[..start]);
        let statement = &remaining[start..];
        let Some(end) = statement.find(';') else {
            return true;
        };
        remaining = &statement[end + 1..];
    }
    stripped.push_str(remaining);

    stripped.contains("Win32::")
        || stripped.contains("windows_sys")
        || stripped.contains("winapi::")
        || stripped.contains("extern \"system\"")
        || stripped.contains("extern \"stdcall\"")
        || stripped.contains("#[link")
}

#[test]
fn night_vision_stays_outside_game_process_and_input_boundaries() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = rust_files(&manifest.join("src/night_vision"));
    files.push(manifest.join("src/night_vision.rs"));
    files.push(manifest.join("src/bin/verify_night_vision.rs"));

    let forbidden = [
        "OpenProcess",
        "ReadProcessMemory",
        "WriteProcessMemory",
        "VirtualAllocEx",
        "CreateRemoteThread",
        "GetProcAddress",
        "LoadLibrary",
        "SetWindowsHookEx",
        "BitBlt",
        "PrintWindow",
        "DesktopDuplication",
        "SendInput",
        "keybd_event",
        "mouse_event",
        "WinSock",
        "pcap",
    ];
    let mut matches = Vec::new();
    for path in &files {
        let source = fs::read_to_string(path).expect("read night vision source");
        for name in forbidden {
            if source.contains(name) {
                matches.push(format!("{}: {name}", path.display()));
            }
        }
    }
    assert!(
        matches.is_empty(),
        "night vision crossed the anti-cheat safety boundary:\n{}",
        matches.join("\n")
    );

    let windows_path = manifest.join("src/night_vision/windows.rs");
    let recovery_path = manifest.join("src/night_vision/recovery.rs");
    let windows_source = fs::read_to_string(&windows_path).expect("read Windows gamma adapter");
    for required in [
        "MonitorFromWindow",
        "GetMonitorInfoW",
        "CreateDCW",
        "GetDeviceGammaRamp",
        "SetDeviceGammaRamp",
        "DeleteDC",
    ] {
        assert!(
            windows_source.contains(required),
            "Windows gamma adapter must retain the audited display API {required}"
        );
    }

    let allowed_gamma_symbols = BTreeSet::from_iter(
        [
            "Foundation",
            "HWND",
            "Graphics",
            "Gdi",
            "CreateDCW",
            "DeleteDC",
            "GetMonitorInfoW",
            "MonitorFromWindow",
            "HDC",
            "MONITORINFOEXW",
            "MONITOR_DEFAULTTONEAREST",
            "UI",
            "ColorSystem",
            "GetDeviceGammaRamp",
            "SetDeviceGammaRamp",
        ]
        .into_iter()
        .map(str::to_string),
    );
    assert_eq!(
        win32_symbols(&windows_source),
        allowed_gamma_symbols,
        "Windows gamma adapter may import only the audited display API allowlist"
    );
    assert!(
        !has_unapproved_windows_boundary(&windows_source),
        "Windows gamma adapter contains a Win32 or manual FFI path outside audited imports"
    );

    let recovery_source = fs::read_to_string(&recovery_path).expect("read recovery file adapter");
    let allowed_recovery_symbols = BTreeSet::from_iter(
        [
            "Storage",
            "FileSystem",
            "MoveFileExW",
            "MOVEFILE_REPLACE_EXISTING",
            "MOVEFILE_WRITE_THROUGH",
        ]
        .into_iter()
        .map(str::to_string),
    );
    assert_eq!(
        win32_symbols(&recovery_source),
        allowed_recovery_symbols,
        "recovery adapter may import only the audited atomic-file API allowlist"
    );
    assert!(
        !has_unapproved_windows_boundary(&recovery_source),
        "recovery adapter contains a Win32 or manual FFI path outside audited imports"
    );

    for path in files
        .iter()
        .filter(|path| *path != &windows_path && *path != &recovery_path)
    {
        let source = fs::read_to_string(path).expect("read night vision source");
        assert!(
            win32_symbols(&source).is_empty(),
            "Win32 API import escaped the two audited adapters into {}",
            path.display()
        );
        assert!(
            !has_unapproved_windows_boundary(&source),
            "unapproved Windows or manual FFI boundary escaped into {}",
            path.display()
        );
        for display_api in ["GetDeviceGammaRamp", "SetDeviceGammaRamp", "CreateDCW"] {
            assert!(
                !source.contains(display_api),
                "display mutation API {display_api} escaped the narrow adapter into {}",
                path.display()
            );
        }
    }

    println!("night vision forbidden API matches: 0; Win32 imports match allowlists");
}

#[test]
fn visual_filter_is_static_click_through_frontend_with_explicit_capability() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let frontend = manifest.join("../src/night-vision-filter");
    let files = [frontend.join("main.ts"), frontend.join("style.css")];
    let forbidden = [
        "getDisplayMedia",
        "getUserMedia",
        "captureStream",
        "canvas",
        "WebSocket",
        "fetch(",
        "XMLHttpRequest",
        "requestPointerLock",
        "dispatchEvent(new KeyboardEvent",
        "dispatchEvent(new MouseEvent",
    ];

    for path in files {
        let source = fs::read_to_string(&path).expect("read visual filter frontend asset");
        for name in forbidden {
            assert!(
                !source.contains(name),
                "visual filter must stay static and offline; found {name} in {}",
                path.display()
            );
        }
    }

    let capability: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(manifest.join("capabilities/default.json"))
            .expect("read default capability"),
    )
    .expect("default capability must be JSON");
    let windows = capability["windows"]
        .as_array()
        .expect("default capability windows array");
    assert!(
        windows.iter().any(|label| label == "night-vision-filter"),
        "night-vision-filter must have the same explicit local capability boundary"
    );
}

#[test]
fn alternate_win32_and_manual_ffi_bypass_forms_are_rejected() {
    for bypass in [
        "fn bypass() { windows::Win32::System::Threading::OpenProcess(); }",
        "use windows::{Win32::System::Threading::OpenProcess};",
        "use windows as w; fn bypass() { w::Win32::System::Threading::OpenProcess(); }",
        "unsafe extern \"system\" { fn OpenProcess(); }",
        "#[link(name = \"kernel32\")] unsafe extern \"C\" { fn OpenProcess(); }",
        "fn bypass() { windows_sys::Win32::System::Threading::OpenProcess(); }",
        "fn bypass() { winapi::um::processthreadsapi::OpenProcess(); }",
    ] {
        assert!(
            has_unapproved_windows_boundary(bypass),
            "safety boundary accepted bypass fixture: {bypass}"
        );
    }
}

#[test]
fn every_normal_exit_path_restores_night_vision_first() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rust_shell = fs::read_to_string(manifest.join("src/lib.rs")).expect("read Rust shell");
    let frontend =
        fs::read_to_string(manifest.join("../src/main/App.svelte")).expect("read app shell");

    assert!(rust_shell.contains("RunEvent::ExitRequested"));
    assert!(rust_shell.contains("night_vision::restore_before_exit"));
    assert!(
        rust_shell.contains("api.prevent_exit()"),
        "native exit must be blocked while display restore remains unverified"
    );
    let prepare = frontend
        .find("await prepareNightVisionExit()")
        .expect("relaunch path must prepare gamma restore");
    let abort = frontend
        .find("if (restored.applied)")
        .expect("relaunch path must stop while display restore remains unverified");
    let relaunch = frontend.find("await relaunch()").expect("relaunch call");
    assert!(
        prepare < abort && abort < relaunch,
        "gamma restore and its failure gate must run before relaunch"
    );
}

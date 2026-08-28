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

    for path in files.iter().filter(|path| *path != &windows_path) {
        let source = fs::read_to_string(path).expect("read night vision source");
        for display_api in ["GetDeviceGammaRamp", "SetDeviceGammaRamp", "CreateDCW"] {
            assert!(
                !source.contains(display_api),
                "display mutation API {display_api} escaped the narrow adapter into {}",
                path.display()
            );
        }
    }

    println!("night vision forbidden API matches: 0");
}

use serde_json::Value;

fn tauri_config() -> Value {
    serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json must be JSON")
}

#[test]
fn updater_endpoint_belongs_to_the_public_fork() {
    let config = tauri_config();
    let endpoints = config["plugins"]["updater"]["endpoints"]
        .as_array()
        .expect("updater endpoints must be an array");

    assert_eq!(
        endpoints,
        &[Value::String(
            "https://github.com/nguyenduytamgithub/theisle-overlay/releases/latest/download/latest.json"
                .to_owned()
        )],
        "the community build must never offer the incompatible upstream 2.x channel"
    );
}

#[test]
fn manual_release_does_not_require_an_updater_private_key() {
    let config = tauri_config();

    assert_eq!(
        config["bundle"]["createUpdaterArtifacts"],
        Value::Bool(false),
        "the fork publishes a manual NSIS release and has no updater signing key"
    );
}

#[test]
fn integrated_candidate_versions_are_consistently_1_9_0() {
    let package: Value = serde_json::from_str(include_str!("../../package.json"))
        .expect("package.json must be JSON");

    assert_eq!(env!("CARGO_PKG_VERSION"), "1.9.0");
    assert_eq!(package["version"], Value::String("1.9.0".to_owned()));
    assert_eq!(tauri_config()["version"], Value::String("1.9.0".to_owned()));

    let runtime = include_str!("../src/night_vision.rs");
    assert!(runtime.contains("gpu-visibility-c"));
    assert!(!runtime.contains("magnifier-boost-b"));
}

#[test]
fn gpu_visibility_copy_discloses_capture_boundary_and_fallback_truthfully() {
    let readme_vi = include_str!("../../README.md");
    let readme_en = include_str!("../../README.en.md");
    let copy_vi = include_str!("../../src/lib/i18n/vi.ts");
    let copy_en = include_str!("../../src/lib/i18n/en.ts");
    let combined = format!("{readme_vi}\n{readme_en}\n{copy_vi}\n{copy_en}");

    assert!(combined.contains("Windows Graphics Capture"));
    assert!(combined.contains("gpu_adaptive"));
    assert!(combined.contains("magnifier_fallback"));
    assert!(combined.contains("Windowed/Borderless"));
    assert!(combined.contains("phím X"));
    assert!(combined.contains("game X key"));
    assert!(combined.contains("không thay đổi ngày/đêm của server"));
    assert!(combined.contains("does not change the server's day/night"));
    assert!(combined.contains("https://github.com/toantranct/theisle-overlay"));
    assert!(combined.contains("https://github.com/nguyenduytamgithub/theisle-overlay"));
    for rejected in [
        "Lớp sáng tĩnh",
        "static light layer",
        "Painted click-through brightness layer",
        "painted acknowledgement",
        "never captures pixels",
        "v1.7.4 Navigation + Magnifier Night Vision",
    ] {
        assert!(
            !combined.contains(rejected),
            "rejected v1.7.1 flat-tint copy remains: {rejected}"
        );
    }
}

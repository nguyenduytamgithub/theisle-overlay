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
fn visual_boost_candidate_versions_are_consistently_1_7_1() {
    let package: Value = serde_json::from_str(include_str!("../../package.json"))
        .expect("package.json must be JSON");

    assert_eq!(env!("CARGO_PKG_VERSION"), "1.7.1");
    assert_eq!(package["version"], Value::String("1.7.1".to_owned()));
    assert_eq!(
        tauri_config()["version"],
        Value::String("1.7.1".to_owned())
    );
}

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


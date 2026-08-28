use theisle_overlay_lib::{night_vision, settings, win};

fn main() {
    let Some(hwnd) = win::game_window::find_game_window(settings::GAME_PROCESS_NAME) else {
        eprintln!("The Isle game window is not running");
        std::process::exit(2);
    };

    match night_vision::run_machine_probe(hwnd, 70, 1_500) {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("night vision probe failed: {error}");
            std::process::exit(1);
        }
    }
}

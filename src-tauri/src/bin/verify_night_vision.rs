use theisle_overlay_lib::{night_vision, settings, win};

fn main() {
    let arguments = std::env::args().collect::<Vec<_>>();
    let gpu_smoke = arguments.iter().any(|argument| argument == "--gpu-smoke");
    let explicit_hwnd = arguments
        .windows(2)
        .find(|pair| pair[0] == "--hwnd")
        .and_then(|pair| pair[1].parse::<isize>().ok());
    let hwnd = explicit_hwnd.or_else(|| win::game_window::find_game_window(settings::GAME_PROCESS_NAME));
    let Some(hwnd) = hwnd else {
        eprintln!("The Isle game window is not running and --hwnd was not supplied");
        std::process::exit(2);
    };

    let result = if gpu_smoke {
        night_vision::run_gpu_visibility_probe(hwnd, 85, 3_000)
            .and_then(|report| serde_json::to_value(report).map_err(|error| error.to_string()))
    } else {
        night_vision::run_machine_probe(hwnd, 70, 1_500)
            .and_then(|report| serde_json::to_value(report).map_err(|error| error.to_string()))
    };

    match result {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => {
            eprintln!("night vision probe failed: {error}");
            std::process::exit(1);
        }
    }
}

use std::env;
use std::process;

fn main() {
    let json_mode = env::args().any(|arg| arg == "--json");
    match star_daemon::run_args(env::args().skip(1)) {
        Ok(output) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
            );
        }
        Err(message) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&star_daemon::error_output(&message))
                        .unwrap_or_else(|_| "{}".to_string())
                );
            } else {
                eprintln!("star-daemon: {}", message);
            }
            process::exit(2);
        }
    }
}

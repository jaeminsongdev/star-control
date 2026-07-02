use star_control_cli::{run_cli, CliConfig};
use std::path::PathBuf;

fn main() {
    let repo_root = std::env::var_os("STAR_CONTROL_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let config = CliConfig::new(repo_root);
    let result = run_cli(std::env::args().skip(1), &config);
    if !result.stdout.is_empty() {
        println!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprintln!("{}", result.stderr);
    }
    std::process::exit(result.exit_code);
}

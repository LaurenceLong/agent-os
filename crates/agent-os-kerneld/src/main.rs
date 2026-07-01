use agent_os_kerneld::run_stdio_daemon;
use std::io;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(error) = run_stdio_daemon(std::env::args().skip(1), stdin.lock(), stdout.lock()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

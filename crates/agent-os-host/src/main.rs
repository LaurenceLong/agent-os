use agent_os_host::run_stdio_host;
use std::io;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if let Err(error) = run_stdio_host(std::env::args().skip(1), stdin.lock(), stdout.lock()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

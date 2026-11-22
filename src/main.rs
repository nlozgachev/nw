use clap::Parser;
use nw::cli::Cli;
use nw::error::NwError;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let stdout_is_tty = stdout.is_terminal();
    let mut stdout_lock = stdout.lock();
    let stderr = io::stderr();
    let mut stderr_lock = stderr.lock();
    let today = chrono::Local::now().date_naive();

    match nw::run_with_io(
        cli,
        &mut reader,
        &mut stdout_lock,
        &mut stderr_lock,
        today,
        stdout_is_tty,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(NwError::Aborted) => {
            let _ = writeln!(stderr_lock, "Aborted.");
            ExitCode::from(130)
        }
        Err(err) => {
            let _ = writeln!(stderr_lock, "error: {err}");
            ExitCode::FAILURE
        }
    }
}

use packs::packs::cli;
use std::process::ExitCode;

pub fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // ViolationsFound already printed its output; other errors need display
            if e.downcast_ref::<cli::ViolationsFound>().is_none() {
                eprintln!("Error: {e:#}");
            }
            // Exit 1 for all application errors (violations, config, IO, etc.)
            // Usage errors are handled by clap with exit code 2
            ExitCode::from(1)
        }
    }
}

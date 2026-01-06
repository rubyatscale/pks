use packs::packs::cli;
use std::process::ExitCode;

pub fn main() -> ExitCode {
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if e.downcast_ref::<cli::ViolationsFound>().is_some() {
                // ViolationsFound already printed its output; exit 1 for violations
                ExitCode::from(1)
            } else {
                // Other errors (IO, config, etc.) exit 2; usage errors handled by clap
                eprintln!("Error: {e:#}");
                ExitCode::from(2)
            }
        }
    }
}

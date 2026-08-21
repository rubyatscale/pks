use packs::packs::cli;
use std::process::ExitCode;

pub fn main() -> ExitCode {
    let result = cli::run();
    // Everything owned by `cli::run` (Configuration, PackSet, the included-file
    // set) has been dropped by this point. On a large codebase that teardown is
    // not free, so it gets its own `--debug` marker.
    tracing::debug!("cli::run returned; owned data dropped");
    match result {
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

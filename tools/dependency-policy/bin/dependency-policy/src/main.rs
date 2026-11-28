//! Command-line entry point for the dependency maintenance gate.

use std::process::ExitCode;

/// Reports maintenance findings and distinguishes policy and operational failures.
fn main() -> ExitCode {
    match dependency_policy::run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("Dependency policy could not be evaluated: {error}");
            ExitCode::from(2)
        }
    }
}

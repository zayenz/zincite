use std::process::ExitCode;

fn main() -> ExitCode {
    zincite_lint::cli::run(std::env::args_os().skip(1).collect(), "zincite-lint")
}

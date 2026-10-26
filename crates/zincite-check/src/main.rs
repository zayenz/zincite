fn main() -> std::process::ExitCode {
    zincite_check::cli::run(std::env::args_os().skip(1).collect(), "zincite-check")
}

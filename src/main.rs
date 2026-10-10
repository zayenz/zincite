use std::io::{self, Write};
use std::process::ExitCode;

const HELP: &str = "Usage: zincite <COMMAND> [OPTIONS] [INPUTS]

MiniZinc source tools.

Commands:
    fmt     Format source, check formatting or write explicit files
    lint    Report modelling advice, inspect rules or apply explicit fixes
    query   Select source items by kind/name, emit source or count items

Use zincite <COMMAND> --help for command options.
";

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(command) = arguments.next() else {
        return print_help();
    };
    match command.to_str() {
        Some("fmt") => zincite_fmt::cli::run(arguments.collect(), "zincite fmt"),
        Some("lint") => zincite_lint::cli::run(arguments.collect(), "zincite lint"),
        Some("query") => zincite_query::cli::run(arguments.collect(), "zincite query"),
        Some("-h" | "--help") => print_help(),
        _ => {
            let _ = writeln!(
                io::stderr(),
                "zincite: unknown command '{}'; use --help for usage",
                command.to_string_lossy()
            );
            ExitCode::from(2)
        }
    }
}

fn print_help() -> ExitCode {
    match io::stdout().write_all(HELP.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "stdout: {error}");
            ExitCode::from(2)
        }
    }
}

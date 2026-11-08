//! Native command-line entry point for type-tune.

use std::process::ExitCode;

mod commands;
pub(crate) mod commit;
pub(crate) mod help;
pub(crate) mod options;
pub(crate) mod output;
pub(crate) mod preview;
pub(crate) mod project;
pub(crate) mod review;
pub(crate) mod scan;

use help::print_help;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("{}", type_tune_core::version());
            ExitCode::SUCCESS
        }
        None | Some("--help" | "-h") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("scan") => commands::run_scan(),
        Some("plan") => commands::run_plan(),
        Some("review") => review::run_review(),
        Some("apply") => commands::run_apply(),
        Some("extract") => commands::run_extract(),
        Some(command) => {
            eprintln!("Unknown command: {command}");
            print_help();
            ExitCode::from(2)
        }
    }
}

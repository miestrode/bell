use cli::RunResult;

use std::process;

fn main() {
    match cli::run() {
        RunResult::Success => process::exit(0),
        RunResult::Failure => process::exit(1),
    }
}

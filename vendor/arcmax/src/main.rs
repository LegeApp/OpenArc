//! arcmax CLI — create, extract, list, and verify .arc archives.
//!
//! Non-interactive:
//!   arcmax create  <output.arc> <path...> [--method zstd] [--level 22] [--block 268435456]
//!   arcmax extract <input.arc>  [dest-dir]
//!   arcmax list    <input.arc>
//!   arcmax verify  <input.arc>
//!
//! Interactive (no args): wizard that prompts for operation and paths.

mod cli;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

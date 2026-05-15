//! arcmax - Simplified FreeARC compression tool
//!
//! This is a clean, simple interface to FreeARC compression algorithms
//! using the successfully built C++ library.

use anyhow::Result;

// Declare our modules
mod cli;

fn main() -> Result<()> {
    cli::dispatch()
}

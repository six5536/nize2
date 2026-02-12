// Import and re-export the `error` module
pub use self::error::{Error, Result};
mod error;

use clap::Parser;
use cli::{Cli, Commands};

mod cli;
mod logging;

fn main() -> Result<()> {
    if let Err(e) = run() {
        log::error!("{}", e);
        std::process::exit(1);
    }
    Ok(())
}

fn run() -> Result<()> {
    logging::init()?;

    let args = Cli::parse();

    match &args.command {
        Commands::Version => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

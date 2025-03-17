#![deny(unused)]

mod cli;

mod errors;
pub use errors::{Error, Result};

mod current_state;
pub use current_state::{CurrentState, CurrentStateTuple};

mod monitor;
pub use monitor::{Monitor, MonitorTuple};

mod printable_monitor;

mod property_map_ext;
pub use property_map_ext::PropertyMapExt;

mod connection;
pub use connection::connect;

mod display_config_proxy;
pub use display_config_proxy::*;

mod structs;
pub use structs::*;

use clap::Parser as _;
use cli::{Cli, DisplayCommand};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    // Handle status
    if let DisplayCommand::Status { modes } = &args.command {
        CurrentState::current(10)
            .await?
            .print_status(*modes)
            .await?;
        return Ok(());
    }

    // Extract rules from command
    let rules = args.command.rules()?;

    if args.test {
        println!("=== TEST MODE ===");
        println!("Changes will be previewed but not applied.\n");
    }

    // If watch flag is enabled
    if args.watch {
        // Start watching for monitor changes
        CurrentState::watch_and_execute(&rules, args.test).await?;
        return Ok(());
    }

    // Execute the selected mode
    CurrentState::determine_and_execute_mode(&rules, 10, args.test).await?;

    Ok(())
}

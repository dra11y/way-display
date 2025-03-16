#![deny(unused)]

mod cli;

mod current_state;
pub use current_state::{CurrentState, CurrentStateTuple};

mod monitor;
pub use monitor::{Monitor, MonitorTuple};

mod printable_monitor;

mod property_map_ext;
pub use property_map_ext::PropertyMapExt;

mod display_config_proxy;
pub use display_config_proxy::*;

mod structs;
pub use structs::*;

use std::time::Duration;

use anyhow::Result;
use clap::Parser as _;
use cli::{Cli, DisplayCommand};
use tokio::time::sleep;
use zbus::Connection;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    // Connect to the session bus
    let mut attempts = 0;
    let connection = loop {
        attempts += 1;
        match Connection::session().await {
            Ok(connection) => break connection,
            Err(error) => {
                eprintln!("Failed to connect to session DBus (attempt {attempts}): {error}");
                if args.watch && attempts < 10 {
                    sleep(Duration::from_secs(1)).await
                } else {
                    return Err(error.into());
                }
            }
        }
    };

    let proxy = DisplayConfigProxy::new(&connection).await?;

    // Get current state
    let current_state: CurrentState = proxy.get_current_state().await?.into();

    // Handle status
    if let DisplayCommand::Status { modes } = &args.command {
        current_state.print_status(*modes).await?;
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
        current_state
            .watch_and_execute(&rules, &connection, args.test)
            .await?;
        return Ok(());
    }

    // Execute the selected mode
    current_state
        .determine_and_execute_mode(&rules, &connection, 10, args.test)
        .await?;

    Ok(())
}

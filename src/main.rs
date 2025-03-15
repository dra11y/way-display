mod display_config;
mod structs;

use std::collections::HashMap;

use anyhow::{Context as AnyhowContext, Result};
use clap::{Parser, Subcommand};
use display_config::DisplayConfigProxy;
use zbus::{Connection, zvariant::OwnedValue};

use structs::{CurrentState, Monitor};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Use only the external monitor if connected
    External,
    /// Use only the internal monitor
    Internal,
    /// Display current monitor configuration
    Status,
    /// Enable both internal and external monitors
    Both,
}

async fn display_status(current_state: &CurrentState) -> Result<()> {
    println!("=== Current Monitor Status ===");

    let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
        current_state.monitors.iter().partition(Monitor::is_builtin);

    println!("Internal Monitors: {}", internal_monitors.len());
    for (i, monitor) in internal_monitors.iter().enumerate() {
        println!("  {}. {}", i + 1, monitor.display_name);
        println!("     Connector: {}", monitor.connector_info.connector);
        println!(
            "     Vendor/Product: {}/{}",
            monitor.connector_info.vendor, monitor.connector_info.product
        );
        if let Some(mode) = monitor.modes.iter().find(|m| m.is_current) {
            println!(
                "     Current Mode: {}x{} @ {:.2}Hz",
                mode.width, mode.height, mode.refresh_rate
            );
        }
    }

    println!("\nExternal Monitors: {}", external_monitors.len());
    for (i, monitor) in external_monitors.iter().enumerate() {
        println!("  {}. {}", i + 1, monitor.display_name);
        println!("     Connector: {}", monitor.connector_info.connector);
        println!(
            "     Vendor/Product: {}/{}",
            monitor.connector_info.vendor, monitor.connector_info.product
        );
        if let Some(mode) = monitor.modes.iter().find(|m| m.is_current) {
            println!(
                "     Current Mode: {}x{} @ {:.2}Hz",
                mode.width, mode.height, mode.refresh_rate
            );
        }
    }

    println!(
        "\nLogical Monitors: {}",
        current_state.logical_monitors.len()
    );
    for (i, logical) in current_state.logical_monitors.iter().enumerate() {
        println!(
            "  {}. Position: ({}, {}), Scale: {}",
            i + 1,
            logical.x,
            logical.y,
            logical.scale
        );
        println!("     Primary: {}", logical.primary);
        for monitor in &logical.assigned_monitors {
            println!("     Connected: {}", monitor.connector);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Connect to the session bus
    let connection = Connection::session()
        .await
        .context("Failed to connect to session DBus")?;

    let proxy = DisplayConfigProxy::new(&connection).await?;

    // Get current state
    let current_state: CurrentState = proxy.get_current_state().await?.into();

    // Execute command
    match args.command {
        Command::Status => {
            display_status(&current_state).await?;
        }
        Command::Internal => {
            println!("Switching to internal monitor...");
            enable_monitors(&connection, &current_state, true, false).await?;
        }
        Command::External => {
            println!("Switching to external monitor...");
            enable_monitors(&connection, &current_state, false, true).await?;
        }
        Command::Both => {
            println!("Enabling both internal and external monitors...");
            enable_monitors(&connection, &current_state, true, true).await?;
        }
    }

    Ok(())
}

async fn enable_monitors(
    connection: &Connection,
    current_state: &CurrentState,
    use_internal: bool,
    use_external: bool,
) -> Result<()> {
    // Partition monitors into internal and external
    let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
        current_state.monitors.iter().partition(|m| m.is_builtin);

    if use_internal && internal_monitors.is_empty() {
        println!("No internal monitors available.");
        return Ok(());
    }

    if use_external && external_monitors.is_empty() {
        println!("No external monitors available.");
        return Ok(());
    }

    // Select which monitors to use
    let monitors_to_use: Vec<_> = match (use_internal, use_external) {
        (true, true) => current_state.monitors.iter().collect(),
        (true, false) => internal_monitors,
        (false, true) => external_monitors,
        (false, false) => {
            println!("No monitors selected to use.");
            return Ok(());
        }
    };

    if monitors_to_use.is_empty() {
        println!("No monitors to configure.");
        return Ok(());
    }

    // Generate logical monitor configurations (side by side)
    let mut configs = Vec::new();
    let mut current_x = 0;

    for (i, monitor) in monitors_to_use.iter().enumerate() {
        // Find best mode for monitor
        if let Some(mode) = monitor
            .modes
            .iter()
            .find(|m| m.is_preferred)
            .or_else(|| monitor.modes.first())
        {
            // Create monitor assignment tuple
            let monitor_assignment = (
                monitor.connector_info.connector.clone(), // connector
                mode.id.clone(),                          // mode_id
                HashMap::<String, OwnedValue>::new(),     // properties
            );

            // Create logical monitor config
            let logical_config = (
                current_x,                // x
                0,                        // y
                mode.preferred_scale,     // scale
                0u32,                     // transform (0 = normal)
                i == 0,                   // primary (first monitor is primary)
                vec![monitor_assignment], // monitors (without properties for logical monitor)
            );

            // Add to configurations
            configs.push(logical_config);

            // Update position for next monitor
            current_x += mode.width;
        }
    }

    if configs.is_empty() {
        println!("Failed to create any valid monitor configurations.");
        return Ok(());
    }

    // Prepare method call parameters
    let method_name = "ApplyMonitorsConfig";
    let path = "/org/gnome/Mutter/DisplayConfig";
    let interface = "org.gnome.Mutter.DisplayConfig";

    // Properties for the overall config
    let config_properties = HashMap::<String, OwnedValue>::new();

    // Parameters for ApplyMonitorsConfig
    let params = (
        current_state.serial, // serial
        1u32,                 // method (1 = temporary, 2 = persistent)
        configs,              // logical monitor configs
        config_properties,    // properties
    );

    // Call the D-Bus method directly
    connection
        .call_method(
            Some("org.gnome.Mutter.DisplayConfig"),
            path,
            Some(interface),
            method_name,
            &params,
        )
        .await?;

    println!("Monitor configuration applied successfully!");
    Ok(())
}

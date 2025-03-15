mod display_config;
mod structs;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context as AnyhowContext, Result};
use clap::{Args, Parser, Subcommand};
use display_config::DisplayConfigProxy;
use futures::StreamExt as _;
use zbus::{Connection, zvariant::OwnedValue};

use structs::{CurrentState, Monitor};

const WATCHING: &str = "\nWatching for monitor configuration changes... (Press Ctrl+C to exit)\n";

/// A tool to manage monitor configurations in Wayland environments
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
struct CliArgs {
    /// Watch for monitor configuration changes and apply rules based on connected monitors
    #[arg(short, long)]
    watch: bool,

    /// Optional configuration file with display rules
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Display commands to execute, in order of preference
    #[command(subcommand)]
    command: DisplayCommand,
}

#[derive(Debug, Subcommand, Clone)]
enum DisplayCommand {
    /// Show current monitor configuration
    Status,

    /// Use only the external monitor if connected
    External(MonitorPattern),

    /// Use only the internal monitor
    Internal(MonitorPattern),

    /// Enable internal and external monitors side by side
    Join(MonitorPattern),

    /// Mirror internal and external monitors
    Mirror(MonitorPattern),

    /// Run multiple rules in sequence (first match wins)
    #[command(alias = "rules")]
    Auto {
        /// Optional descriptive name for this rule set
        #[arg(short, long)]
        name: Option<String>,

        /// Use external display when pattern matches
        #[arg(long, value_name = "PATTERN")]
        external: Vec<String>,

        /// Use internal display when pattern matches
        #[arg(long, value_name = "PATTERN")]
        internal: Vec<String>,

        /// Use join displays when pattern matches
        #[arg(long, value_name = "PATTERN")]
        join: Vec<String>,

        /// Use mirrored displays when pattern matches
        #[arg(long, value_name = "PATTERN")]
        mirror: Vec<String>,

        /// Default mode if no patterns match
        #[arg(long, value_enum, default_value = "external")]
        default: DefaultMode,
    },
}

#[derive(Debug, Args, Clone, Default)]
struct MonitorPattern {
    /// Match by connector name (e.g., DP-6, HDMI-1)
    #[arg(long)]
    connector: Option<String>,

    /// Match by vendor name (e.g., ACR, DEL)
    #[arg(long)]
    vendor: Option<String>,

    /// Match by product name (e.g., ET430K)
    #[arg(long)]
    product: Option<String>,

    /// Match by serial number
    #[arg(long)]
    serial: Option<String>,

    /// Match by display name (e.g., "Acer Technologies 42\"")
    #[arg(long)]
    name: Option<String>,
}

impl MonitorPattern {
    fn is_empty(&self) -> bool {
        self.connector.is_none()
            && self.vendor.is_none()
            && self.product.is_none()
            && self.serial.is_none()
            && self.name.is_none()
    }

    fn matches(&self, monitor: &Monitor) -> bool {
        // If no patterns are specified, this is a default rule that always matches
        if self.is_empty() {
            return true;
        }

        // Check each specified pattern - all must match
        (match &self.connector {
            None => true,
            Some(pattern) => monitor.connector_info.connector.contains(pattern),
        }) && (match &self.vendor {
            None => true,
            Some(pattern) => monitor.connector_info.vendor.contains(pattern),
        }) && (match &self.product {
            None => true,
            Some(pattern) => monitor.connector_info.product.contains(pattern),
        }) && (match &self.serial {
            None => true,
            Some(pattern) => monitor.connector_info.serial.contains(pattern),
        }) && (match &self.name {
            None => true,
            Some(pattern) => monitor.display_name.contains(pattern),
        })
    }

    fn from_string(pattern: &str) -> Self {
        // Parse patterns like "connector=DP-6", "product=Acer", etc.
        let parts: Vec<&str> = pattern.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Self {
                name: Some(pattern.to_string()),
                ..Default::default()
            };
        }

        let field = parts[0].trim();
        let value = parts[1].trim().to_string();

        match field {
            "connector" => Self {
                connector: Some(value),
                ..Default::default()
            },
            "vendor" => Self {
                vendor: Some(value),
                ..Default::default()
            },
            "product" => Self {
                product: Some(value),
                ..Default::default()
            },
            "serial" => Self {
                serial: Some(value),
                ..Default::default()
            },
            "name" => Self {
                name: Some(value),
                ..Default::default()
            },
            _ => Self {
                name: Some(pattern.to_string()),
                ..Default::default()
            },
        }
    }
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
        println!("     Serial: {}", monitor.connector_info.serial);
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
        println!("     Serial: {}", monitor.connector_info.serial);
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

#[derive(Debug, Clone)]
enum DisplayMode {
    External,
    Internal,
    Join,
    Mirror,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DefaultMode {
    External,
    Internal,
    Join,
    Mirror,
}

#[derive(Debug, Clone)]
struct DisplayRule {
    mode: DisplayMode,
    pattern: MonitorPattern,
}

impl DisplayRule {
    fn matches_any_monitor(&self, monitors: &[&Monitor]) -> bool {
        monitors.iter().any(|monitor| self.pattern.matches(monitor))
    }
}

fn get_rules_from_command(command: &DisplayCommand) -> Vec<DisplayRule> {
    match command {
        DisplayCommand::Status => vec![],
        DisplayCommand::External(pattern) => vec![DisplayRule {
            mode: DisplayMode::External,
            pattern: pattern.clone(),
        }],
        DisplayCommand::Internal(pattern) => vec![DisplayRule {
            mode: DisplayMode::Internal,
            pattern: pattern.clone(),
        }],
        DisplayCommand::Join(pattern) => vec![DisplayRule {
            mode: DisplayMode::Join,
            pattern: pattern.clone(),
        }],
        DisplayCommand::Mirror(pattern) => vec![DisplayRule {
            mode: DisplayMode::Mirror,
            pattern: pattern.clone(),
        }],
        DisplayCommand::Auto {
            external,
            internal,
            join,
            mirror,
            default,
            ..
        } => {
            let mut rules = Vec::new();

            // Add mirror rules
            for pattern_str in mirror {
                rules.push(DisplayRule {
                    mode: DisplayMode::Mirror,
                    pattern: MonitorPattern::from_string(pattern_str),
                });
            }

            // Add join rules
            for pattern_str in join {
                rules.push(DisplayRule {
                    mode: DisplayMode::Join,
                    pattern: MonitorPattern::from_string(pattern_str),
                });
            }

            // Add external rules
            for pattern_str in external {
                rules.push(DisplayRule {
                    mode: DisplayMode::External,
                    pattern: MonitorPattern::from_string(pattern_str),
                });
            }

            // Add internal rules
            for pattern_str in internal {
                rules.push(DisplayRule {
                    mode: DisplayMode::Internal,
                    pattern: MonitorPattern::from_string(pattern_str),
                });
            }

            // Add the default rule (always matches)
            rules.push(DisplayRule {
                mode: match default {
                    DefaultMode::External => DisplayMode::External,
                    DefaultMode::Internal => DisplayMode::Internal,
                    DefaultMode::Join => DisplayMode::Join,
                    DefaultMode::Mirror => DisplayMode::Mirror,
                },
                pattern: MonitorPattern::default(),
            });

            rules
        }
    }
}

fn determine_mode(rules: &[DisplayRule], current_state: &CurrentState) -> Option<DisplayMode> {
    // Check each external monitor against the rules
    let external_monitors: Vec<_> = current_state
        .monitors
        .iter()
        .filter(|m| !m.is_builtin)
        .collect();

    // Default to external if no rules provided and monitors are available
    if rules.is_empty() {
        if external_monitors.is_empty() {
            return Some(DisplayMode::Internal);
        } else {
            return Some(DisplayMode::External);
        }
    }

    // Go through rules in order (first match wins)
    for rule in rules {
        // Check if any monitor matches the pattern
        if rule.pattern.is_empty() || rule.matches_any_monitor(&external_monitors) {
            return Some(rule.mode.clone());
        }
    }

    // No rules matched - use internal if no external monitors, otherwise external
    if external_monitors.is_empty() {
        Some(DisplayMode::Internal)
    } else {
        Some(DisplayMode::External)
    }
}

async fn execute_mode(
    connection: &Connection,
    mode: &DisplayMode,
    current_state: &CurrentState,
) -> Result<()> {
    match mode {
        DisplayMode::Internal => {
            println!("Switching to internal monitor...");
            enable_monitors(connection, current_state, true, false, false).await?;
        }
        DisplayMode::External => {
            println!("Switching to external monitor...");
            enable_monitors(connection, current_state, false, true, false).await?;
        }
        DisplayMode::Join => {
            println!("Joining internal and external monitors...");
            enable_monitors(connection, current_state, true, true, false).await?;
        }
        DisplayMode::Mirror => {
            println!("Mirroring internal and external monitors...");
            enable_monitors(connection, current_state, true, true, true).await?;
        }
    }
    Ok(())
}

async fn watch_and_execute(
    connection: &Connection,
    rules: &[DisplayRule],
    current_state: &CurrentState,
) -> Result<()> {
    let proxy = DisplayConfigProxy::new(connection).await?;

    // Create a stream to receive the MonitorsChanged signal
    let mut stream = proxy.receive_monitors_changed().await?;

    println!("{WATCHING}");

    let mut monitors = current_state.monitors.clone();

    // Poll for signal events
    while (stream.next().await).is_some() {
        // Get the updated state
        let current_state: CurrentState = proxy.get_current_state().await?.into();

        if current_state.monitors == monitors {
            continue;
        }

        monitors = current_state.monitors.clone();

        println!("Monitor configuration changed!");

        // Determine which mode applies based on the connected monitors
        if let Some(mode) = determine_mode(rules, &current_state) {
            // Execute the selected mode
            execute_mode(connection, &mode, &current_state).await?;
        } else {
            println!("No rules provided");
        }

        println!("{WATCHING}");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    // Connect to the session bus
    let connection = Connection::session()
        .await
        .context("Failed to connect to session DBus")?;

    let proxy = DisplayConfigProxy::new(&connection).await?;

    // Get current state
    let current_state: CurrentState = proxy.get_current_state().await?.into();

    // If this is a status command, just display and exit
    if let DisplayCommand::Status = &args.command {
        display_status(&current_state).await?;
        return Ok(());
    }

    // Extract rules from command
    let rules = get_rules_from_command(&args.command);

    // Determine which mode applies based on the connected monitors
    if let Some(mode) = determine_mode(&rules, &current_state) {
        // Execute the selected mode
        execute_mode(&connection, &mode, &current_state).await?;
    } else {
        println!("No applicable rules found");
        return Ok(());
    }

    // If --watch flag is enabled, start watching for monitor changes
    if args.watch {
        watch_and_execute(&connection, &rules, &current_state).await?;
    }

    Ok(())
}

async fn enable_monitors(
    connection: &Connection,
    current_state: &CurrentState,
    use_internal: bool,
    use_external: bool,
    mirror_mode: bool,
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
        (true, false) => internal_monitors.clone(),
        (false, true) => external_monitors.clone(),
        (false, false) => {
            println!("No monitors selected to use.");
            return Ok(());
        }
    };

    if monitors_to_use.is_empty() {
        println!("No monitors to configure.");
        return Ok(());
    }

    // Generate logical monitor configurations
    let mut configs = Vec::new();

    if mirror_mode && monitors_to_use.len() > 1 {
        // For mirror mode, create a single logical monitor with all physical monitors

        // Find a reference monitor - try to use external monitors' mode as reference if available
        let reference_monitor = if !external_monitors.is_empty() {
            external_monitors[0]
        } else {
            monitors_to_use[0]
        };

        // Find modes that all monitors support (same resolution)
        // First, collect all the resolutions from the reference monitor
        let reference_resolutions: Vec<(i32, i32)> = reference_monitor
            .modes
            .iter()
            .map(|m| (m.width, m.height))
            .collect();

        // Find a resolution that all monitors support
        let mut compatible_resolution = None;

        // Start with trying to find the best preferred resolution
        for &(width, height) in &reference_resolutions {
            let all_monitors_support = monitors_to_use.iter().all(|monitor| {
                monitor
                    .modes
                    .iter()
                    .any(|m| m.width == width && m.height == height)
            });

            if all_monitors_support {
                compatible_resolution = Some((width, height));

                // If this is a preferred resolution, prioritize it
                let is_preferred = reference_monitor
                    .modes
                    .iter()
                    .any(|m| m.width == width && m.height == height && m.is_preferred);

                if is_preferred {
                    break;
                }
            }
        }

        // If no compatible resolution found, we can't mirror
        let (common_width, common_height) = match compatible_resolution {
            Some(res) => res,
            None => {
                println!("Error: Could not find a common resolution for all monitors to mirror.");
                println!("Try using 'join' mode instead, or configure only certain monitors.");
                return Ok(());
            }
        };

        println!(
            "Using common resolution for mirroring: {}x{}",
            common_width, common_height
        );

        // Create monitor assignments for all monitors with the same resolution
        let monitor_assignments: Vec<_> = monitors_to_use
            .iter()
            .map(|monitor| {
                // Find the mode with matching resolution
                let mode = monitor
                    .modes
                    .iter()
                    .find(|m| m.width == common_width && m.height == common_height)
                    .expect("Monitor should have the common resolution mode");

                (
                    monitor.connector_info.connector.clone(), // connector
                    mode.id.clone(),                          // mode_id
                    HashMap::<String, OwnedValue>::new(),     // properties
                )
            })
            .collect();

        // Get the scale from reference monitor's mode with this resolution
        let reference_scale = reference_monitor
            .modes
            .iter()
            .find(|m| m.width == common_width && m.height == common_height)
            .map(|m| m.preferred_scale)
            .unwrap_or(1.0);

        // Create a single logical monitor for all physical monitors
        configs.push((
            0,                   // x
            0,                   // y
            reference_scale,     // scale
            0u32,                // transform (0 = normal)
            true,                // primary
            monitor_assignments, // all monitors assigned to same logical monitor
        ));
    } else {
        // For join mode (side by side), use previous logic with scaling fix
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

                // Calculate logical width considering the scale factor
                let logical_width = (mode.width as f64 / mode.preferred_scale).round() as i32;

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

                // Update position for next monitor using logical width
                current_x += logical_width;
            }
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

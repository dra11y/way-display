mod display_config;
mod structs;

use std::collections::HashMap;

use anyhow::{Context as AnyhowContext, Result};
use clap::{Args, Parser, Subcommand};
use display_config::DisplayConfigProxy;
use futures::StreamExt as _;
use zbus::{Connection, zvariant::OwnedValue};

use structs::{ConnectorInfo, CurrentState, Monitor};

const WATCHING: &str = "\nWatching for monitor configuration changes... (Press Ctrl+C to exit)\n";

/// Manage display (monitor) selection in Wayland environments.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
struct CliArgs {
    /// Watch for monitor configuration changes and apply rules based on connected monitors
    #[arg(short, long)]
    watch: bool,

    /// TODO: (Not yet implemented)
    /// Optional configuration file with display rules
    // #[arg(short, long)]
    // config: Option<PathBuf>,

    /// Display commands to execute, in order of preference
    #[command(subcommand)]
    command: DisplayCommand,
}

#[derive(Debug, Subcommand, Clone)]
enum DisplayCommand {
    /// Show current monitor configuration (--modes to include display modes)
    Status {
        /// Include detailed information about available display modes
        #[arg(short, long)]
        modes: bool,
    },

    /// Use only the external monitor (if connected)
    External(MonitorPattern),

    /// Use only the internal monitor (if exists)
    Internal(MonitorPattern),

    /// Enable internal and external monitors side by side
    Join(MonitorPattern),

    /// Mirror internal and external monitors (uses the highest resolution common mode)
    Mirror(MonitorPattern),

    /// Test pattern matching against current monitors
    #[command(arg_required_else_help = true)]
    Test(MonitorPattern),

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
        default: DisplayMode,
    },
}

#[derive(Debug, Args, Clone, Default)]
struct MonitorPattern {
    /// Exact match by connector name (e.g., DP-6, HDMI-1)
    #[arg(long)]
    connector: Option<String>,

    /// Exact match by vendor code (e.g., ACR, DEL)
    #[arg(long)]
    vendor: Option<String>,

    /// Partial or exact match by product name (e.g., "ET430K" or "Acer ET430K")
    #[arg(long)]
    product: Option<String>,

    /// Partial or exact match by serial number (e.g., "0x714" or "0x7140025c")
    #[arg(long)]
    serial: Option<String>,

    /// Partial or exact match by display name (e.g., "Acer" or "Acer Technologies 42")
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
            Some(pattern) => monitor.connector_info.connector == *pattern,
        }) && (match &self.vendor {
            None => true,
            Some(pattern) => monitor.connector_info.vendor == *pattern,
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

fn print_connector_info(i: Option<usize>, connector_info: &ConnectorInfo) {
    let (line_0, line_n) = match i {
        Some(i) => (format!("{}. ", i + 1), "   "),
        None => ("".to_string(), ""),
    };
    println!("     {line_0}Connector: {}", connector_info.connector);
    println!("     {line_n}Vendor: {}", connector_info.vendor);
    println!("     {line_n}Product: {}", connector_info.product);
    println!("     {line_n}Serial: {}", connector_info.serial);
}

fn print_monitor(i: usize, monitor: &Monitor) {
    println!("  {}. {}", i + 1, monitor.display_name);
    print_connector_info(None, &monitor.connector_info);
    if let Some(mode) = monitor.modes.iter().find(|m| m.is_current) {
        println!(
            "     Current Mode: {}x{} @ {:.2}Hz",
            mode.width, mode.height, mode.refresh_rate
        );
    }
}

fn print_modes(monitor: &Monitor) {
    println!("     Available Modes (* = current, P = preferred):");

    // Create a vector of references to the modes for sorting
    let mut modes = monitor.modes.iter().collect::<Vec<_>>();

    // Sort modes by resolution (width*height) in descending order, then by refresh rate
    modes.sort_by(|a, b| {
        let a_pixels = a.width * a.height;
        let b_pixels = b.width * b.height;

        // First sort by resolution (descending)
        b_pixels
            .cmp(&a_pixels)
            // Then by refresh rate (descending)
            .then(
                b.refresh_rate
                    .partial_cmp(&a.refresh_rate)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    for (i, mode) in modes.iter().enumerate() {
        let current_marker = if mode.is_current { "*" } else { " " };
        let preferred_marker = if mode.is_preferred { "P" } else { " " };

        println!(
            "     {current_marker} {preferred_marker} {:2}. {}x{} @ {:.2}Hz",
            i + 1,
            mode.width,
            mode.height,
            mode.refresh_rate,
        );

        // Print scales if they exist and differ from 1.0
        if !mode.supported_scales.is_empty()
            && (mode.preferred_scale != 1.0 || mode.supported_scales.iter().any(|&s| s != 1.0))
        {
            print!("             Scales: {:.2}", mode.preferred_scale);

            // Print other supported scales if they differ from preferred
            let other_scales: Vec<_> = mode
                .supported_scales
                .iter()
                .filter(|&&s| (s - mode.preferred_scale).abs() > 0.01)
                .collect();

            for scale in other_scales {
                print!(", {:.2}", scale);
            }
            println!();
        }
    }
}

async fn display_status(current_state: &CurrentState, show_modes: bool) -> Result<()> {
    println!("=== Current Monitor Status ===");

    let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
        current_state.monitors.iter().partition(Monitor::is_builtin);

    println!("Internal Monitors: {}", internal_monitors.len());
    for (i, monitor) in internal_monitors.iter().enumerate() {
        print_monitor(i, monitor);
        if show_modes {
            print_modes(monitor);
        }
    }

    println!("\nExternal Monitors: {}", external_monitors.len());
    for (i, monitor) in external_monitors.iter().enumerate() {
        print_monitor(i, monitor);
        if show_modes {
            print_modes(monitor);
        }
    }

    println!(
        "\nLogical Monitors: {}",
        current_state.logical_monitors.len()
    );
    for (i, logical) in current_state.logical_monitors.iter().enumerate() {
        println!("  {}. Position: ({}, {})", i + 1, logical.x, logical.y,);
        println!("     Scale: {}", logical.scale);
        println!("     Primary: {}", logical.primary);
        println!("     Transform: {}", logical.transform);
        println!("     Assigned Monitors:");
        for (i, connector_info) in logical.assigned_monitors.iter().enumerate() {
            print_connector_info(Some(i), connector_info);
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum DisplayMode {
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
        DisplayCommand::Test(_) => unreachable!(),
        DisplayCommand::Status { .. } => unreachable!(),
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
                mode: *default,
                pattern: MonitorPattern::default(),
            });

            rules
        }
    }
}

fn determine_mode(rules: &[DisplayRule], current_state: &CurrentState) -> Result<DisplayMode> {
    // Check each external monitor against the rules
    let external_monitors: Vec<_> = current_state
        .monitors
        .iter()
        .filter(|m| !m.is_builtin)
        .collect();

    // Default to external if no rules provided and monitors are available
    if rules.is_empty() {
        if external_monitors.is_empty() {
            return Ok(DisplayMode::Internal);
        } else {
            return Ok(DisplayMode::External);
        }
    }

    // Go through rules in order (first match wins)
    for rule in rules {
        // For non-empty patterns, ensure we have a matching monitor
        if !rule.pattern.is_empty() {
            // Check if any monitor matches the pattern
            let has_match = rule.matches_any_monitor(&external_monitors);

            if has_match {
                return Ok(rule.mode);
            }

            // Continue to the next rule if no match
            continue;
        }

        // For empty patterns (default rules), check if the mode is valid with current monitors
        match rule.mode {
            DisplayMode::External => {
                if external_monitors.is_empty() {
                    // Skip this rule - we can't use external mode without external monitors
                    continue;
                }
                return Ok(DisplayMode::External);
            }
            DisplayMode::Internal => {
                if current_state.monitors.iter().any(|m| m.is_builtin) {
                    return Ok(DisplayMode::Internal);
                }
                // Skip if no internal monitor
                continue;
            }
            DisplayMode::Join => {
                if external_monitors.is_empty()
                    || !current_state.monitors.iter().any(|m| m.is_builtin)
                {
                    // Need both internal and external for join mode
                    continue;
                }
                return Ok(DisplayMode::Join);
            }
            DisplayMode::Mirror => {
                if external_monitors.is_empty()
                    || !current_state.monitors.iter().any(|m| m.is_builtin)
                {
                    // Need both internal and external for mirror mode
                    continue;
                }
                return Ok(DisplayMode::Mirror);
            }
        }
    }

    // No rules matched
    Err(anyhow::anyhow!(
        "No matching monitor configuration found for the specified rules"
    ))
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
        match determine_mode(rules, &current_state) {
            Ok(mode) => {
                // Execute the selected mode
                execute_mode(connection, &mode, &current_state).await?;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1); // Exit with error status
            }
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

    // Handle different commands
    match &args.command {
        DisplayCommand::Status { modes } => {
            display_status(&current_state, *modes).await?;
            return Ok(());
        }
        DisplayCommand::Test(pattern) => {
            test_pattern_matching(pattern, &current_state).await?;
            return Ok(());
        }
        _ => {}
    }

    // Extract rules from command
    let rules = get_rules_from_command(&args.command);

    // Determine which mode applies based on the connected monitors
    match determine_mode(&rules, &current_state) {
        Ok(mode) => {
            // Execute the selected mode
            execute_mode(&connection, &mode, &current_state).await?;
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1); // Exit with error status
        }
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
        (true, false) => internal_monitors.clone(), // Clone to avoid move
        (false, true) => external_monitors.clone(), // Clone to avoid move
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

        // Find a reference monitor - prefer external monitors as they typically have better resolution
        let reference_monitor = if !external_monitors.is_empty() {
            external_monitors[0]
        } else {
            monitors_to_use[0]
        };

        // Collect all resolutions that every monitor supports
        let mut common_resolutions: Vec<(i32, i32)> = Vec::new();

        // Start with all resolutions from the reference monitor
        for mode in &reference_monitor.modes {
            let resolution = (mode.width, mode.height);

            // Check if all monitors support this resolution
            let all_support = monitors_to_use.iter().all(|monitor| {
                monitor
                    .modes
                    .iter()
                    .any(|m| m.width == resolution.0 && m.height == resolution.1)
            });

            if all_support {
                common_resolutions.push(resolution);
            }
        }

        // Sort resolutions by total pixels (highest resolution first)
        common_resolutions.sort_by(|a, b| {
            let a_pixels = a.0 * a.1;
            let b_pixels = b.0 * b.1;
            b_pixels.cmp(&a_pixels) // Descending order
        });

        // If no common resolutions found, we can't mirror
        if common_resolutions.is_empty() {
            println!("Error: Could not find a common resolution for all monitors to mirror.");
            println!("Try using 'join' mode instead, or configure only certain monitors.");
            return Ok(());
        }

        // Use the highest resolution that all monitors support
        let (common_width, common_height) = common_resolutions[0];

        println!(
            "Using highest common resolution for mirroring: {}x{}",
            common_width, common_height
        );

        // Create monitor assignments for all monitors with the same resolution
        let monitor_assignments: Vec<_> = monitors_to_use
            .iter()
            .map(|monitor| {
                // Find the mode with matching resolution
                // If multiple modes with same resolution exist (different refresh rates),
                // prefer the one with highest refresh rate
                let mode = monitor
                    .modes
                    .iter()
                    .filter(|m| m.width == common_width && m.height == common_height)
                    .max_by(|a, b| {
                        a.refresh_rate
                            .partial_cmp(&b.refresh_rate)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .expect("Monitor should have the common resolution mode");

                (
                    monitor.connector_info.connector.clone(), // connector
                    mode.id.clone(),                          // mode_id
                    HashMap::<String, OwnedValue>::new(),     // properties
                )
            })
            .collect();

        // Get the scale from reference monitor's mode with this resolution - prefer 1.0 scale if possible
        let reference_scale = reference_monitor
            .modes
            .iter()
            .find(|m| m.width == common_width && m.height == common_height)
            .map(|m| {
                if m.preferred_scale > 1.0 {
                    m.preferred_scale
                } else {
                    1.0
                }
            })
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

async fn test_pattern_matching(
    pattern: &MonitorPattern,
    current_state: &CurrentState,
) -> Result<()> {
    println!("=== Testing Pattern Matching ===");

    // Print the pattern being tested
    println!("Testing pattern:");
    if let Some(connector) = &pattern.connector {
        println!("  Connector: {}", connector);
    }
    if let Some(vendor) = &pattern.vendor {
        println!("  Vendor: {}", vendor);
    }
    if let Some(product) = &pattern.product {
        println!("  Product: {}", product);
    }
    if let Some(serial) = &pattern.serial {
        println!("  Serial: {}", serial);
    }
    if let Some(name) = &pattern.name {
        println!("  Display Name: {}", name);
    }

    println!("\nResults:");

    let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
        current_state.monitors.iter().partition(Monitor::is_builtin);

    // Check internal monitors
    println!("\nInternal Monitors:");
    if internal_monitors.is_empty() {
        println!("  None found");
    } else {
        let matching_internal: Vec<_> = internal_monitors
            .iter()
            .filter(|m| pattern.matches(m))
            .collect();

        if matching_internal.is_empty() {
            println!("  All {} filtered out", internal_monitors.len());
        } else {
            for (i, monitor) in matching_internal.iter().enumerate() {
                println!(
                    "  {}. {} ({})",
                    i + 1,
                    monitor.display_name,
                    monitor.connector_info.connector
                );
            }

            let filtered_count = internal_monitors.len() - matching_internal.len();
            if filtered_count > 0 {
                println!("  {} filtered out", filtered_count);
            } else {
                println!("  None filtered out");
            }
        }
    }

    // Check external monitors
    println!("\nExternal Monitors:");
    if external_monitors.is_empty() {
        println!("  None found");
    } else {
        let matching_external: Vec<_> = external_monitors
            .iter()
            .filter(|m| pattern.matches(m))
            .collect();

        if matching_external.is_empty() {
            println!("  All {} filtered out", external_monitors.len());
        } else {
            for (i, monitor) in matching_external.iter().enumerate() {
                println!(
                    "  {}. {} ({})",
                    i + 1,
                    monitor.display_name,
                    monitor.connector_info.connector
                );
            }

            let filtered_count = external_monitors.len() - matching_external.len();
            if filtered_count > 0 {
                println!("  {} filtered out", filtered_count);
            } else {
                println!("  None filtered out");
            }
        }
    }

    // Summary
    let total_matches = current_state
        .monitors
        .iter()
        .filter(|m| pattern.matches(m))
        .count();
    println!(
        "\nSummary: {} of {} monitors matched the pattern",
        total_matches,
        current_state.monitors.len()
    );

    Ok(())
}

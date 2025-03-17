use std::{collections::HashMap, time::Duration};

use futures::StreamExt as _;
use tokio::time::sleep;
use zbus::zvariant::OwnedValue;

use crate::{
    DisplayConfigProxy, Error, Monitor, Result,
    cli::{DisplayMode, DisplayRule},
    connect,
    printable_monitor::convert_for_printing,
    structs::{ApplyLogicalMonitorTuple, ConnectorInfo, CurrentLogicalMonitor},
};

const WATCHING: &str = "\nWatching for monitor configuration changes... (Press Ctrl+C to exit)\n";

#[derive(Debug, Clone)]
pub struct CurrentState {
    pub serial: u32,
    pub monitors: Vec<Monitor>,
    pub logical_monitors: Vec<CurrentLogicalMonitor>,
    // pub properties: HashMap<String, OwnedValue>,
}

impl CurrentState {
    pub async fn current(max_attempts: usize) -> Result<Self> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            if attempt > 1 {
                sleep(Duration::from_secs(1)).await;
                if attempt >= max_attempts {
                    return Err(Error::MaxAttempts(max_attempts));
                }
            }

            let connection = match connect(10).await {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("Attempt {attempt}: Failed to connect to DBus: {error}");
                    continue;
                }
            };

            let proxy = match DisplayConfigProxy::new(&connection).await {
                Ok(proxy) => proxy,
                Err(error) => {
                    eprintln!(
                        "Attempt {attempt}: Failed to connect to DisplayConfigProxy: {error}"
                    );
                    continue;
                }
            };

            match proxy.get_current_state().await {
                Ok(state) => return Ok(state.into()),
                Err(error) => {
                    eprintln!("Attempt {attempt}: DBus Proxy Error: {error}");
                    continue;
                }
            }
        }
    }

    pub fn print_connector_info(&self, i: Option<usize>, connector_info: &ConnectorInfo) {
        let (line_0, line_n) = match i {
            Some(i) => (format!("{}. ", i + 1), "   "),
            None => ("".to_string(), ""),
        };
        println!("     {line_0}Connector: {}", connector_info.connector);

        // Find the monitor in current state
        if let Some(monitor) = self
            .monitors
            .iter()
            .find(|m| m.connector_info.connector == *connector_info.connector)
        {
            println!("        Display: {}", monitor.display_name);

            // Find the mode info
            if let Some(mode) = monitor.modes.iter().find(|m| m.is_current) {
                println!(
                    "        Mode: {}x{} @ {:.2}Hz",
                    mode.width, mode.height, mode.refresh_rate
                );
            }
        }

        println!("     {line_n}Vendor: {}", connector_info.vendor);
        println!("     {line_n}Product: {}", connector_info.product);
        println!("     {line_n}Serial: {}", connector_info.serial);
    }

    pub fn print_monitor(&self, i: usize, monitor: &Monitor, show_modes: bool) {
        println!("  {}. {}", i + 1, monitor.display_name);
        self.print_connector_info(None, &monitor.connector_info);
        if let Some(mode) = monitor.modes.iter().find(|m| m.is_current) {
            println!(
                "     Current Mode: {}x{} @ {:.2}Hz",
                mode.width, mode.height, mode.refresh_rate
            );
        }
        if show_modes {
            monitor.print_modes();
        }
    }

    pub fn print_logical_monitor(&self, i: usize, logical: &CurrentLogicalMonitor) {
        println!("  {}. Position: ({}, {})", i + 1, logical.x, logical.y,);
        println!("     Scale: {}", logical.scale);
        println!("     Primary: {}", logical.primary);
        println!("     Transform: {}", logical.transform);
        println!("     Assigned Monitors:");
        for (i, connector_info) in logical.assigned_monitors.iter().enumerate() {
            self.print_connector_info(Some(i), connector_info);
        }
    }

    pub async fn print_status(&self, show_modes: bool) -> Result<()> {
        println!("=== Current Monitor Status ===");

        let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
            self.monitors.iter().partition(Monitor::is_builtin);

        println!("Internal Monitors: {}", internal_monitors.len());
        for (i, monitor) in internal_monitors.iter().enumerate() {
            self.print_monitor(i, monitor, show_modes);
        }

        println!("\nExternal Monitors: {}", external_monitors.len());
        for (i, monitor) in external_monitors.iter().enumerate() {
            self.print_monitor(i, monitor, show_modes);
        }

        println!("\nLogical Monitors: {}", self.logical_monitors.len());
        for (i, logical) in self.logical_monitors.iter().enumerate() {
            self.print_logical_monitor(i, logical);
        }

        Ok(())
    }

    pub async fn enable_monitors(mode: &DisplayMode, attempt: usize, dry_run: bool) -> Result<()> {
        let state = Self::current(10).await?;

        // Partition monitors into internal and external
        let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
            state.monitors.iter().partition(|m| m.is_builtin);

        let monitors_to_use = match mode {
            DisplayMode::External => {
                if external_monitors.is_empty() {
                    eprintln!("No external monitors available.");
                    return Ok(());
                }
                external_monitors
            }
            DisplayMode::Internal => {
                if internal_monitors.is_empty() {
                    eprintln!("No internal monitors available.");
                    return Ok(());
                }
                internal_monitors
            }
            DisplayMode::Join | DisplayMode::Mirror => {
                if state.monitors.is_empty() {
                    eprintln!("No monitors to configure.");
                    return Ok(());
                }
                state.monitors.iter().collect()
            }
        };

        // Generate logical monitor configurations

        let logical_monitors: Vec<ApplyLogicalMonitorTuple> = match mode {
            DisplayMode::Mirror => build_mirrored(monitors_to_use),
            _ => build_joined_or_individual(monitors_to_use, mode),
        }?;

        if logical_monitors.is_empty() {
            return Err(Error::NoMonitorsAvailable(*mode));
        }

        if dry_run {
            println!("[TEST MODE] The following configuration would have been applied:");
            for (i, logical) in logical_monitors.iter().enumerate() {
                let print_monitor = convert_for_printing(logical, &state.monitors);
                print_monitor.print(i);
            }
            return Ok(());
        }

        let method_name = "ApplyMonitorsConfig";
        let path = "/org/gnome/Mutter/DisplayConfig";
        let interface = "org.gnome.Mutter.DisplayConfig";

        let config_properties = HashMap::<String, OwnedValue>::new();

        // Parameters for ApplyMonitorsConfig
        let params = (
            state.serial,             // serial
            1u32,                     // method (1 = temporary, 2 = persistent)
            logical_monitors.clone(), // logical monitor configs
            config_properties,        // properties
        );

        println!("Connecting to DBus (attempt {attempt})...");
        let connection = connect(10).await?;

        let message = connection
            .call_method(
                Some("org.gnome.Mutter.DisplayConfig"),
                path,
                Some(interface),
                method_name,
                &params,
            )
            .await?;

        let updated_state = CurrentState::current(10).await?;
        match updated_state.verify_applied_config(&logical_monitors) {
            Ok(true) => {
                println!("✓ Monitor configuration successfully applied.");
                Ok(())
            }
            Ok(false) => Err(Error::FailedVerification(message)),
            Err(error) => Err(error),
        }
    }

    pub async fn determine_and_execute_mode(
        rules: &[DisplayRule],
        attempt: usize,
        dry_run: bool,
    ) -> Result<()> {
        let mut inner_attempt = 0;
        loop {
            inner_attempt += 1;
            println!("Attempt {inner_attempt} of 3: Determine mode and execute...");
            if inner_attempt > 1 {
                sleep(Duration::from_secs(1)).await;
            }

            let mode = match Self::determine_mode(rules).await {
                Ok(mode) => mode,
                Err(Error::NoMonitorsMatch(_)) => {
                    eprintln!("No monitors match rules, returning OK.");
                    return Ok(());
                }
                Err(error) => {
                    if inner_attempt < 3 {
                        continue;
                    }
                    return Err(error);
                }
            };

            println!("Determined mode: {mode:?}");

            match Self::enable_monitors(&mode, attempt, dry_run).await {
                Ok(_) => return Ok(()),
                Err(error) => {
                    if inner_attempt < 3 {
                        continue;
                    }
                    return Err(error);
                }
            }
        }
    }

    pub async fn watch_and_execute(rules: &[DisplayRule], dry_run: bool) -> Result<()> {
        let mut attempt = 0;
        'outer: loop {
            attempt += 1;
            if attempt > 1 {
                sleep(Duration::from_secs(1)).await;
            }
            eprintln!("Watch attempt: {attempt}");

            // let current_state = Self::current(10).await?;

            let connection = match connect(10).await {
                Ok(connection) => connection,
                Err(error) => {
                    eprintln!("Watch attempt {attempt}: Failed to connect to DBus: {error}");
                    continue;
                }
            };

            let proxy = match DisplayConfigProxy::new(&connection).await {
                Ok(proxy) => proxy,
                Err(error) => {
                    eprintln!("Failed to connect to proxy: {error}");
                    continue;
                }
            };

            // Create a stream to receive the MonitorsChanged signal
            let mut stream = match proxy.receive_monitors_changed().await {
                Ok(stream) => stream,
                Err(error) => {
                    eprintln!("Failed to get monitor stream: {error}");
                    continue;
                }
            };

            // Execute the selected mode
            match Self::determine_and_execute_mode(rules, attempt, dry_run).await {
                Ok(_) => (),
                Err(Error::ZBus(error)) => {
                    eprintln!("ZBus error: {error}, retrying...");
                    continue 'outer;
                }
                Err(Error::NoMonitorsMatch(_)) => (),
                Err(error) => {
                    println!("Failed to apply INITIAL display configuration: {}", error);
                    continue 'outer;
                }
            }

            println!("{}", WATCHING);

            let mut monitors = Self::current(10).await?.monitors.clone();

            // Poll for signal events
            while (stream.next().await).is_some() {
                // Get the updated state
                let updated_state: CurrentState = proxy.get_current_state().await?.into();

                if updated_state.monitors == monitors {
                    continue;
                }

                println!("Monitor configuration changed!");

                monitors = updated_state.monitors.clone();

                // Execute the selected mode
                match Self::determine_and_execute_mode(rules, attempt, dry_run).await {
                    Ok(_) => (),
                    Err(error) => {
                        eprintln!("Failed to apply CHANGED display configuration: {error}");
                        eprintln!("Restarting outer loop...");
                        continue 'outer;
                    }
                }

                println!("{}", WATCHING);
            }
        }
    }

    async fn determine_mode(rules: &[DisplayRule]) -> Result<DisplayMode> {
        let state = Self::current(10).await?;

        if state.monitors.is_empty() {
            return Err(Error::NoMonitorsAvailable(DisplayMode::Internal));
        }

        // Check each external monitor against the rules
        let (internal_monitors, external_monitors): (Vec<_>, Vec<_>) =
            state.monitors.iter().partition(|m| m.is_builtin);

        // Default to external if no rules provided and monitors are available
        if rules.is_empty() {
            if external_monitors.is_empty() {
                return Ok(DisplayMode::Internal);
            } else {
                return Ok(DisplayMode::External);
            }
        }

        // For single-rule commands (External, Internal, etc.), handle appropriately
        if rules.len() == 1 {
            let rule = &rules[0];

            // Check if we need to match against external or internal monitors based on the mode
            let monitors_to_check = match rule.mode {
                DisplayMode::External => &external_monitors,
                DisplayMode::Internal => &internal_monitors,
                // For modes requiring both types, check all monitors
                _ => &state.monitors.iter().collect::<Vec<_>>(),
            };

            if monitors_to_check.is_empty() {
                return Err(Error::NoMonitorsAvailable(rule.mode));
            }

            // If pattern is not empty, check if any monitor matches the pattern
            if !rule.pattern.is_empty() {
                let has_match = monitors_to_check
                    .iter()
                    .any(|monitor| rule.pattern.matches(monitor));

                if !has_match {
                    return Err(Error::NoMonitorsMatch(rules.to_vec()));
                }
            }

            // For modes requiring both monitor types, make sure both exist
            match rule.mode {
                DisplayMode::Join | DisplayMode::Mirror => {
                    let len = state.monitors.len();
                    if len < 2 {
                        return Err(Error::InsufficientMonitorsAvailable {
                            available: len,
                            required: 2,
                            mode: rule.mode,
                        });
                    }
                }
                _ => {}
            }

            return Ok(rule.mode);
        }

        // For multi-rule commands (Auto), go through rules in order
        for rule in rules {
            // For non-empty patterns, ensure we have a matching monitor
            if !rule.pattern.is_empty() {
                // For modes requiring specific monitor types, check appropriate collection
                let monitors_to_check = match rule.mode {
                    DisplayMode::External => &external_monitors,
                    DisplayMode::Internal => &internal_monitors,
                    _ => &state.monitors.iter().collect::<Vec<_>>(),
                };

                if monitors_to_check.is_empty() {
                    // Skip this rule - no monitors of required type available
                    continue;
                }

                let has_match = monitors_to_check
                    .iter()
                    .any(|monitor| rule.pattern.matches(monitor));

                if has_match {
                    // For modes requiring both monitor types, ensure both exist
                    match rule.mode {
                        DisplayMode::Join | DisplayMode::Mirror => {
                            if external_monitors.is_empty() || internal_monitors.is_empty() {
                                // Skip this rule - can't use join/mirror without both types
                                continue;
                            }
                        }
                        _ => {}
                    }

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
                    if !internal_monitors.is_empty() {
                        return Ok(DisplayMode::Internal);
                    }
                    // Skip if no internal monitor
                    continue;
                }
                DisplayMode::Join => {
                    if external_monitors.is_empty() || internal_monitors.is_empty() {
                        // Need both internal and external for join mode
                        continue;
                    }
                    return Ok(DisplayMode::Join);
                }
                DisplayMode::Mirror => {
                    if external_monitors.is_empty() || internal_monitors.is_empty() {
                        // Need both internal and external for mirror mode
                        continue;
                    }
                    return Ok(DisplayMode::Mirror);
                }
            }
        }

        // No rules matched
        Err(Error::NoMonitorsMatch(rules.to_vec()))
    }

    /// Verify if the applied configuration matches what we intended to apply
    pub fn verify_applied_config(
        &self,
        intended_logical_monitors: &[ApplyLogicalMonitorTuple],
    ) -> Result<bool> {
        // If count doesn't match, configuration definitely didn't apply correctly
        if self.logical_monitors.len() != intended_logical_monitors.len() {
            println!(
                "Configuration mismatch: Expected {} logical monitors, but found {}",
                intended_logical_monitors.len(),
                self.logical_monitors.len()
            );
            return Ok(false);
        }

        // Check each logical monitor
        for intended in intended_logical_monitors {
            let (
                intended_x,
                intended_y,
                intended_scale,
                intended_transform,
                intended_primary,
                intended_monitors,
            ) = intended;

            // Try to find a matching logical monitor in the current configuration
            let found_match = self.logical_monitors.iter().any(|current| {
                // Check position, scale, transform, and primary status
                if current.x != *intended_x
                    || current.y != *intended_y
                    || (current.scale - *intended_scale).abs() > 0.001
                    || current.transform != *intended_transform
                    || current.primary != *intended_primary
                {
                    return false;
                }

                // Check if the monitor count matches
                if current.assigned_monitors.len() != intended_monitors.len() {
                    return false;
                }

                // Check each connector in the logical monitor
                for (connector, mode_id, _) in intended_monitors {
                    // Find this connector in the current configuration
                    let found_connector = current
                        .assigned_monitors
                        .iter()
                        .any(|m| m.connector == *connector);

                    if !found_connector {
                        return false;
                    }

                    // Find the monitor and check if it's using the intended mode
                    if let Some(monitor) = self
                        .monitors
                        .iter()
                        .find(|m| m.connector_info.connector == *connector)
                    {
                        let using_intended_mode = monitor
                            .modes
                            .iter()
                            .filter(|m| m.is_current)
                            .any(|m| m.id == *mode_id);

                        if !using_intended_mode {
                            return false;
                        }
                    } else {
                        // Monitor not found in current state
                        return false;
                    }
                }

                true
            });

            if !found_match {
                println!(
                    "Configuration mismatch: Could not find matching logical monitor for intended config at position ({}, {})",
                    intended_x, intended_y
                );
                return Ok(false);
            }
        }

        // All checks passed
        Ok(true)
    }
}

pub type CurrentStateTuple = (
    u32,
    Vec<(
        (String, String, String, String),
        Vec<(
            String,
            i32,
            i32,
            f64,
            f64,
            Vec<f64>,
            HashMap<String, OwnedValue>,
        )>,
        HashMap<String, OwnedValue>,
    )>,
    Vec<(
        i32,
        i32,
        f64,
        u32,
        bool,
        Vec<(String, String, String, String)>,
        HashMap<String, OwnedValue>,
    )>,
    HashMap<String, OwnedValue>,
);

impl From<CurrentStateTuple> for CurrentState {
    fn from(value: CurrentStateTuple) -> Self {
        Self {
            serial: value.0,
            monitors: value.1.into_iter().map(Monitor::from).collect(),
            logical_monitors: value
                .2
                .into_iter()
                .map(CurrentLogicalMonitor::from)
                .collect(),
            // properties: value.3,
        }
    }
}

fn build_joined_or_individual(
    monitors_to_use: Vec<&Monitor>,
    mode: &DisplayMode,
) -> Result<Vec<ApplyLogicalMonitorTuple>> {
    // For join mode (side by side), use previous logic with scaling fix
    let mut current_x = 0;

    let mut logical_monitors = vec![];

    for (i, monitor) in monitors_to_use.iter().enumerate() {
        // Find best mode for monitor
        let Some(mode) = monitor
            .modes
            .iter()
            .find(|m| m.is_preferred)
            .or_else(|| monitor.modes.first())
        else {
            continue;
        };

        // Create monitor assignment tuple with the expected format

        let monitor_assignment = (
            monitor.connector_info.connector.clone(), // connector
            mode.id.clone(),                          // mode_id
            HashMap::<String, OwnedValue>::new(),     // properties
        );

        // Calculate logical width considering the scale factor
        let logical_width = (mode.width as f64 / mode.preferred_scale).round() as i32;
        // Create logical monitor config

        let logical_monitor = (
            current_x,                // x
            0,                        // y
            mode.preferred_scale,     // scale
            0u32,                     // transform (0 = normal)
            i == 0,                   // primary (first monitor is primary)
            vec![monitor_assignment], // monitors (without properties for logical monitor)
        );

        // Add to configurations

        logical_monitors.push(logical_monitor);

        // Update position for next monitor using logical width
        current_x += logical_width;
    }

    if logical_monitors.is_empty() {
        return Err(Error::NoMonitorsAvailable(*mode));
    }

    Ok(logical_monitors)
}

fn build_mirrored(monitors_to_use: Vec<&Monitor>) -> Result<Vec<ApplyLogicalMonitorTuple>> {
    // For mirror mode, create a single logical monitor with all physical monitors

    // Find a reference monitor - prefer external monitors as they typically have better resolution
    let reference_monitor = monitors_to_use
        .iter()
        .find(|m| !m.is_builtin)
        .or_else(|| monitors_to_use.first())
        .ok_or(Error::NoMonitorsAvailable(DisplayMode::Mirror))?;

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
        return Err(Error::NoCommonResolutionsAvailable);
    }

    // Use the highest resolution that all monitors support
    let (common_width, common_height) = common_resolutions[0];

    println!(
        "Using highest common resolution for mirroring: {}x{}",
        common_width, common_height
    );

    // Create monitor assignments for all monitors with the same resolution
    let assigned_monitors = monitors_to_use
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
    let scale = reference_monitor
        .modes
        .iter()
        .find(|m| m.width == common_width && m.height == common_height)
        .map(|m| m.preferred_scale.max(1.0))
        .unwrap_or(1.0);

    // Create a single logical monitor for all physical monitors

    Ok(vec![(
        0,                 // x
        0,                 // y
        scale,             // scale
        0u32,              // transform (0 = normal)
        true,              // primary
        assigned_monitors, // all monitors assigned to same logical monitor
    )])
}

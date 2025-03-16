use crate::{ApplyLogicalMonitorTuple, ConnectorInfo, Monitor};

/// TODO: Possible to unify this with [`crate::CurrentLogicalMonitor`]?
/// Converts an ApplyLogicalMonitorTuple into a user-friendly structure for printing
pub fn convert_for_printing(
    logical_monitor: &ApplyLogicalMonitorTuple,
    monitors: &[Monitor],
) -> PrintableLogicalMonitor {
    let (x, y, scale, transform, primary, assigned_monitors) = logical_monitor;

    let print_monitors = assigned_monitors
        .iter()
        .map(|(connector, mode_id, _properties)| {
            // Find the corresponding monitor
            let monitor = monitors
                .iter()
                .find(|m| m.connector_info.connector == *connector);

            // Create a connector info
            let connector_info = match monitor {
                Some(m) => ConnectorInfo {
                    connector: connector.clone(),
                    vendor: m.connector_info.vendor.clone(),
                    product: m.connector_info.product.clone(),
                    serial: m.connector_info.serial.clone(),
                },
                None => ConnectorInfo {
                    connector: connector.clone(),
                    vendor: "Unknown".to_string(),
                    product: "Unknown".to_string(),
                    serial: "Unknown".to_string(),
                },
            };

            // Find the mode details
            let mode_details = monitor
                .and_then(|m| m.modes.iter().find(|mode| mode.id == *mode_id))
                .map(|mode| ModeDetails {
                    width: mode.width,
                    height: mode.height,
                    refresh_rate: mode.refresh_rate,
                    is_preferred: mode.is_preferred,
                })
                .unwrap_or_else(|| ModeDetails {
                    width: 0,
                    height: 0,
                    refresh_rate: 0.0,
                    is_preferred: false,
                });

            PrintableMonitor {
                connector_info,
                display_name: monitor
                    .map(|m| m.display_name.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                mode_details,
            }
        })
        .collect();

    PrintableLogicalMonitor {
        x: *x,
        y: *y,
        scale: *scale,
        transform: *transform,
        primary: *primary,
        assigned_monitors: print_monitors,
    }
}

#[derive(Debug)]
struct ModeDetails {
    width: i32,
    height: i32,
    refresh_rate: f64,
    is_preferred: bool,
}

#[derive(Debug)]
struct PrintableMonitor {
    connector_info: ConnectorInfo,
    display_name: String,
    mode_details: ModeDetails,
}

#[derive(Debug)]
pub struct PrintableLogicalMonitor {
    x: i32,
    y: i32,
    scale: f64,
    transform: u32,
    primary: bool,
    assigned_monitors: Vec<PrintableMonitor>,
}

impl PrintableLogicalMonitor {
    pub fn print(&self, index: usize) {
        println!("  {}. Position: ({}, {})", index + 1, self.x, self.y);
        println!("     Scale: {}", self.scale);
        println!("     Primary: {}", self.primary);
        println!("     Transform: {}", self.transform);
        println!("     Assigned Monitors:");

        for (i, monitor) in self.assigned_monitors.iter().enumerate() {
            println!(
                "     {}. Connector: {}",
                i + 1,
                monitor.connector_info.connector
            );
            println!("        Display: {}", monitor.display_name);
            println!(
                "        Mode: {}x{} @ {:.2}Hz {}",
                monitor.mode_details.width,
                monitor.mode_details.height,
                monitor.mode_details.refresh_rate,
                if monitor.mode_details.is_preferred {
                    "(preferred)"
                } else {
                    ""
                }
            );
            println!("        Vendor: {}", monitor.connector_info.vendor);
            println!("        Product: {}", monitor.connector_info.product);
            println!("        Serial: {}", monitor.connector_info.serial);
        }

        println!(); // Add an empty line between logical monitors
    }
}

use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use crate::{
    PropertyMapExt as _,
    structs::{ConnectorInfo, Mode},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub is_builtin: bool,
    pub is_underscanning: bool,
    pub min_refresh_rate: Option<i32>,
    pub display_name: String,
    pub connector_info: ConnectorInfo,
    pub modes: Vec<Mode>,
    pub properties: HashMap<String, OwnedValue>,
}

impl Monitor {
    pub fn is_builtin(self: &&Self) -> bool {
        self.is_builtin
    }

    pub fn print_modes(&self) {
        println!("     Available Modes (* = current, P = preferred):");

        // Create a vector of references to the modes for sorting
        let mut modes = self.modes.iter().collect::<Vec<_>>();

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
}

pub type MonitorTuple = (
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
);

impl From<MonitorTuple> for Monitor {
    fn from(value: MonitorTuple) -> Self {
        let properties = value.2;
        let is_builtin: bool = properties.get_as("is-builtin").unwrap_or(false);
        let is_underscanning = properties.get_as("is-underscanning").unwrap_or(false);
        let min_refresh_rate = properties.get_as("min-refresh-rate");
        let display_name = properties.get_as("display-name").unwrap_or_default();
        Self {
            is_builtin,
            is_underscanning,
            min_refresh_rate,
            display_name,
            connector_info: ConnectorInfo::from(value.0),
            modes: value.1.into_iter().map(Mode::from).collect(),
            properties,
        }
    }
}

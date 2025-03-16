use std::{convert::Infallible, str::FromStr};

use clap::Args;

use crate::Monitor;

#[derive(Debug, Args, Clone, Default)]
pub struct MonitorPattern {
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
    pub fn is_empty(&self) -> bool {
        self.connector.is_none()
            && self.vendor.is_none()
            && self.product.is_none()
            && self.serial.is_none()
            && self.name.is_none()
    }

    pub fn matches(&self, monitor: &Monitor) -> bool {
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
}

impl FromStr for MonitorPattern {
    type Err = Infallible;

    fn from_str(pattern: &str) -> std::result::Result<Self, Self::Err> {
        // Parse patterns like "connector=DP-6", "product=Acer", etc.
        let parts: Vec<&str> = pattern.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Ok(Self {
                name: Some(pattern.to_string()),
                ..Default::default()
            });
        }

        let field = parts[0].trim();
        let value = parts[1].trim().to_string();

        Ok(match field {
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
        })
    }
}

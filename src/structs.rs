use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

use crate::PropertyMapExt as _;

// ApplyConfiguration is deprecated; use ApplyMonitorsConfig
// https://browse.dgit.debian.org/mutter.git/plain/data/dbus-interfaces/org.gnome.Mutter.DisplayConfig.xml

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorInfo {
    pub connector: String,
    pub vendor: String,
    pub product: String,
    pub serial: String,
}

pub type ConnectorInfoTuple = (String, String, String, String);

impl From<ConnectorInfoTuple> for ConnectorInfo {
    fn from(value: ConnectorInfoTuple) -> Self {
        Self {
            connector: value.0,
            vendor: value.1,
            product: value.2,
            serial: value.3,
        }
    }
}

// GetCurrentState structures
#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    pub id: String,
    pub width: i32,
    pub height: i32,
    pub refresh_rate: f64,
    pub is_current: bool,
    pub is_preferred: bool,
    pub preferred_scale: f64,
    pub supported_scales: Vec<f64>,
    pub properties: HashMap<String, OwnedValue>,
}

pub type ModeTuple = (
    String,
    i32,
    i32,
    f64,
    f64,
    Vec<f64>,
    HashMap<String, OwnedValue>,
);

impl From<ModeTuple> for Mode {
    fn from(value: ModeTuple) -> Self {
        let properties = value.6;
        let is_preferred = properties.get_as("is-preferred").unwrap_or(false);
        let is_current = properties.get_as("is-current").unwrap_or(false);

        Self {
            id: value.0,
            width: value.1,
            height: value.2,
            refresh_rate: value.3,
            is_current,
            is_preferred,
            preferred_scale: value.4,
            supported_scales: value.5,
            properties,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CurrentLogicalMonitor {
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u32,
    pub primary: bool,
    pub assigned_monitors: Vec<ConnectorInfo>,
    // pub properties: HashMap<String, OwnedValue>,
}

pub type ApplyLogicalMonitorTuple = (
    i32,
    i32,
    f64,
    u32,
    bool,
    Vec<(String, String, HashMap<String, OwnedValue>)>,
);

pub type CurrentLogicalMonitorTuple = (
    i32,
    i32,
    f64,
    u32,
    bool,
    Vec<(String, String, String, String)>,
    HashMap<String, OwnedValue>,
);

impl From<CurrentLogicalMonitorTuple> for CurrentLogicalMonitor {
    fn from(value: CurrentLogicalMonitorTuple) -> Self {
        Self {
            x: value.0,
            y: value.1,
            scale: value.2,
            transform: value.3,
            primary: value.4,
            assigned_monitors: value.5.into_iter().map(ConnectorInfo::from).collect(),
            // properties: value.6,
        }
    }
}

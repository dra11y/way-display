use anyhow::Result;
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

// ApplyConfiguration is deprecated; use ApplyMonitorsConfig
// https://browse.dgit.debian.org/mutter.git/plain/data/dbus-interfaces/org.gnome.Mutter.DisplayConfig.xml

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

trait PropertyMapExtensions {
    fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static;

    fn try_get_as<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static;
}

impl PropertyMapExtensions for HashMap<String, OwnedValue> {
    fn get_as<T>(&self, key: &str) -> Option<T>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        self.try_get_as::<T>(key).and_then(|v| v.ok())
    }

    fn try_get_as<T>(&self, key: &str) -> Option<Result<T>>
    where
        T: TryFrom<OwnedValue>,
        T::Error: std::error::Error + Send + Sync + 'static,
    {
        self.get(key)
            .map(|v| T::try_from(v.clone()).map_err(Into::into))
    }
}

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
pub struct CurrentState {
    pub serial: u32,
    pub monitors: Vec<Monitor>,
    pub logical_monitors: Vec<CurrentLogicalMonitor>,
    // pub properties: HashMap<String, OwnedValue>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyLogicalMonitor {
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u32,
    pub primary: bool,
    pub assigned_monitors: Vec<ApplyMonitorAssignment>,
    pub properties: HashMap<String, OwnedValue>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyMonitorAssignment {
    pub connector: String,
    pub mode_id: String,
    pub properties: HashMap<String, OwnedValue>,
}

// Conversion from existing Monitor to MonitorAssignment
impl From<(&Monitor, &Mode)> for ApplyMonitorAssignment {
    fn from((monitor, mode): (&Monitor, &Mode)) -> Self {
        Self {
            connector: monitor.connector_info.connector.clone(),
            mode_id: mode.id.clone(),
            properties: monitor.properties.clone(),
        }
    }
}

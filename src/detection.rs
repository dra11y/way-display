use std::{env, sync::Arc};

use crate::{Error, Result};

#[derive(Debug)]
pub struct DbusConfig {
    pub service: &'static str,
    pub path: &'static str,
    pub interface: &'static str,
    pub method: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DesktopEnvironment {
    Gnome,
    Cinnamon,
    Unknown(Arc<str>),
}

impl DesktopEnvironment {
    pub fn detect() -> Self {
        let xdg_desktop = env::var("XDG_SESSION_DESKTOP")
            .unwrap_or_default()
            .to_lowercase();

        if xdg_desktop.contains("cinnamon") {
            return DesktopEnvironment::Cinnamon;
        }

        if xdg_desktop.contains("gnome") {
            return DesktopEnvironment::Gnome;
        }

        DesktopEnvironment::Unknown(xdg_desktop.into())
    }

    pub fn dbus_config(&self) -> Result<DbusConfig> {
        match self {
            Self::Gnome => Ok(DbusConfig {
                service: "org.gnome.Mutter.DisplayConfig",
                path: "/org/gnome/Mutter/DisplayConfig",
                interface: "org.gnome.Mutter.DisplayConfig",
                method: "ApplyMonitorsConfig",
            }),
            Self::Cinnamon => Ok(DbusConfig {
                service: "org.cinnamon.Muffin.DisplayConfig",
                path: "/org/cinnamon/Muffin/DisplayConfig",
                interface: "org.cinnamon.Muffin.DisplayConfig",
                method: "ApplyMonitorsConfig",
            }),
            Self::Unknown(desktop) => Err(Error::UnsupportedDesktop(desktop.clone())),
        }
    }
}

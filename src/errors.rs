use std::sync::Arc;

use thiserror::Error;

use crate::cli::{DisplayMode, DisplayRule};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),
    #[error("Max attempts ({0}) reached, aborting.")]
    MaxAttempts(usize),
    #[error("No monitors available for display mode: {0}")]
    NoMonitorsAvailable(DisplayMode),
    #[error(
        "Insufficient monitors ({available} available, {required} required) for display mode: {mode}"
    )]
    InsufficientMonitorsAvailable {
        available: usize,
        required: usize,
        mode: DisplayMode,
    },
    #[error("No common resolutions are available among the monitors for mirroring")]
    NoCommonResolutionsAvailable,
    #[error("No monitors match the provided rules: {0:#?}")]
    NoMonitorsMatch(Vec<DisplayRule>),
    #[error("✗ Monitor configuration was attempted but failed verification. Reply message: {0:#?}")]
    FailedVerification(zbus::Message),
    #[error("Unsupported desktop: {0}")]
    UnsupportedDesktop(Arc<str>),
    #[error("ZBus error: {0:#?}")]
    ZBus(#[from] zbus::Error),
    #[error("ZVariant error: {0:#?}")]
    ZVariant(#[from] zbus::zvariant::Error),
}

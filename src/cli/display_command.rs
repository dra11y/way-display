use std::str::FromStr as _;

use anyhow::Result;
use clap::Subcommand;

use super::{DisplayMode, DisplayRule, monitor_pattern::MonitorPattern};

#[derive(Debug, Subcommand, Clone)]
pub enum DisplayCommand {
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

impl DisplayCommand {
    pub fn rules(&self) -> Result<Vec<DisplayRule>> {
        Ok(match self {
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
                        pattern: MonitorPattern::from_str(pattern_str)?,
                    });
                }

                // Add join rules
                for pattern_str in join {
                    rules.push(DisplayRule {
                        mode: DisplayMode::Join,
                        pattern: MonitorPattern::from_str(pattern_str)?,
                    });
                }

                // Add external rules
                for pattern_str in external {
                    rules.push(DisplayRule {
                        mode: DisplayMode::External,
                        pattern: MonitorPattern::from_str(pattern_str)?,
                    });
                }

                // Add internal rules
                for pattern_str in internal {
                    rules.push(DisplayRule {
                        mode: DisplayMode::Internal,
                        pattern: MonitorPattern::from_str(pattern_str)?,
                    });
                }

                // Add the default rule (always matches)
                rules.push(DisplayRule {
                    mode: *default,
                    pattern: MonitorPattern::default(),
                });

                rules
            }
        })
    }
}

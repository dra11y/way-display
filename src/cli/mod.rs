mod display_command;
use clap::Parser;
pub use display_command::DisplayCommand;
mod monitor_pattern;
pub use monitor_pattern::MonitorPattern;
use strum::Display;

/// Manage display (monitor) selection in Wayland environments.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
pub struct Cli {
    /// Watch for monitor configuration changes and apply rules based on connected monitors
    #[arg(short, long)]
    pub watch: bool,

    /// Dry run mode: print what would be done without making changes
    #[arg(short, long)]
    pub test: bool,

    /// TODO: (Not yet implemented)
    /// Optional configuration file with display rules
    // #[arg(short, long)]
    // pub config: Option<PathBuf>,

    /// Display commands to execute, in order of preference
    #[command(subcommand)]
    pub command: DisplayCommand,
}

#[derive(Debug, Clone, Copy, Display, clap::ValueEnum)]
pub enum DisplayMode {
    External,
    Internal,
    Join,
    Mirror,
}

#[derive(Debug, Clone)]
pub struct DisplayRule {
    pub mode: DisplayMode,
    pub pattern: MonitorPattern,
}

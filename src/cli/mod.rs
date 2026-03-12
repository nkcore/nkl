mod handler;

use clap::{Parser, Subcommand};

use crate::config::Config;

/// Replace port numbers with stable, named .localhost URLs.
#[derive(Parser)]
#[command(name = "nkl", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub(super) command: Commands,
}

#[derive(Subcommand)]
pub(super) enum Commands {
    /// Infer name from project and run through proxy
    Run {
        /// Command and arguments to run
        #[arg(trailing_var_arg = true, required = true)]
        cmd: Vec<String>,

        /// Override the auto-inferred project name
        #[arg(long)]
        name: Option<String>,

        /// Use HTTPS/HTTP2
        #[arg(long)]
        https: bool,

        /// Use a fixed port for the app
        #[arg(long)]
        app_port: Option<u16>,

        /// Override a route registered by another process
        #[arg(long)]
        force: bool,

        /// Rewrite Host header to target address
        #[arg(long)]
        change_origin: bool,
    },

    /// Manage the proxy server
    Proxy {
        #[command(subcommand)]
        action: ProxyAction,
    },

    /// Register a static route (e.g. for Docker)
    Alias {
        /// App name
        name: Option<String>,

        /// Target port
        port: Option<u16>,

        /// Remove the alias
        #[arg(long)]
        remove: bool,

        /// Override an existing route
        #[arg(long)]
        force: bool,

        /// Rewrite Host header to target address
        #[arg(long)]
        change_origin: bool,

        /// Path prefix for routing (e.g. /api)
        #[arg(long, default_value = "/")]
        path: String,

        /// Strip the matched path prefix before forwarding
        #[arg(long)]
        strip_prefix: bool,
    },

    /// Output the URL for a given app name (for scripts)
    Get {
        /// App name
        name: String,
    },

    /// Show active routes
    List,

    /// Show proxy and route status with process details
    Status,

    /// Add local CA to system trust store
    Trust,

    /// Show or manage configuration
    Config,

    /// Manage /etc/hosts entries
    Hosts {
        #[command(subcommand)]
        action: HostsAction,
    },
}

#[derive(Subcommand)]
pub(super) enum ProxyAction {
    /// Start the proxy server
    Start {
        /// Port for the proxy (default: from config or 1355)
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable HTTPS/HTTP2
        #[arg(long)]
        https: bool,

        /// Run in foreground (default: daemon mode)
        #[arg(long)]
        foreground: bool,

        /// Internal: daemonize the process before starting the proxy
        #[arg(long, hide = true)]
        daemonize: bool,
    },
    /// Stop the proxy server
    Stop,
    /// Show proxy daemon logs
    Logs {
        /// Follow log output in real time (like tail -f)
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show from the end (default: all)
        #[arg(short = 'n', long)]
        lines: Option<usize>,
    },
}

#[derive(Subcommand)]
pub(super) enum HostsAction {
    /// Add routes to /etc/hosts
    Sync,
    /// Remove nkl entries from /etc/hosts
    Clean,
}

/// Apply CLI flag overrides to a config.
pub(super) fn apply_cli_overrides(config: &mut Config, port: Option<u16>, https: bool) {
    if let Some(p) = port {
        config.proxy_port = p;
    }
    if https {
        config.proxy_https = true;
    }
}

impl Cli {
    pub async fn run(self) -> anyhow::Result<()> {
        handler::handle(self).await
    }
}

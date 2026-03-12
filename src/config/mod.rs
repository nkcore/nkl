use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::utils::{DEFAULT_PORT, PRIVILEGED_PORT_THRESHOLD};

/// Raw TOML config with all fields optional (for layered merging).
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct RawConfig {
    pub proxy: Option<RawProxy>,
    pub app: Option<RawApp>,
    pub hosts: Option<RawHosts>,
    pub paths: Option<RawPaths>,
    pub tls: Option<RawTls>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct RawProxy {
    pub port: Option<u16>,
    pub https: Option<bool>,
    pub max_hops: Option<u8>,
    /// Custom domain suffixes (e.g. ["localhost", "dev.local", "test"]).
    pub domains: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct RawApp {
    pub port: Option<u16>,
    pub port_range_start: Option<u16>,
    pub port_range_end: Option<u16>,
    pub force: Option<bool>,
    pub ready_timeout: Option<u64>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct RawHosts {
    pub auto_sync: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct RawPaths {
    pub state_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct RawTls {
    pub cert: Option<String>,
    pub key: Option<String>,
}

/// Resolved config with concrete values (defaults applied).
#[derive(Debug, Clone)]
pub struct Config {
    pub proxy_port: u16,
    pub proxy_https: bool,
    pub max_hops: u8,
    /// Allowed domain suffixes for hostname registration.
    pub domains: Vec<String>,
    pub app_port: Option<u16>,
    pub app_port_range: (u16, u16),
    pub app_force: bool,
    pub ready_timeout: u64,
    pub hosts_auto_sync: bool,
    pub state_dir: Option<PathBuf>,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            proxy_port: DEFAULT_PORT,
            proxy_https: false,
            max_hops: 5,
            domains: vec!["localhost".to_string()],
            app_port: None,
            app_port_range: (3000, 9999),
            app_force: false,
            ready_timeout: 30,
            hosts_auto_sync: false,
            state_dir: None,
            tls_cert: None,
            tls_key: None,
        }
    }
}

impl RawConfig {
    /// Merge another RawConfig on top of self (other wins on conflicts).
    pub fn merge(self, other: RawConfig) -> RawConfig {
        RawConfig {
            proxy: merge_opt(self.proxy, other.proxy, |a, b| RawProxy {
                port: b.port.or(a.port),
                https: b.https.or(a.https),
                max_hops: b.max_hops.or(a.max_hops),
                domains: b.domains.or(a.domains),
            }),
            app: merge_opt(self.app, other.app, |a, b| RawApp {
                port: b.port.or(a.port),
                port_range_start: b.port_range_start.or(a.port_range_start),
                port_range_end: b.port_range_end.or(a.port_range_end),
                force: b.force.or(a.force),
                ready_timeout: b.ready_timeout.or(a.ready_timeout),
            }),
            hosts: merge_opt(self.hosts, other.hosts, |a, b| RawHosts {
                auto_sync: b.auto_sync.or(a.auto_sync),
            }),
            paths: merge_opt(self.paths, other.paths, |a, b| RawPaths {
                state_dir: b.state_dir.or(a.state_dir),
            }),
            tls: merge_opt(self.tls, other.tls, |a, b| RawTls {
                cert: b.cert.or(a.cert),
                key: b.key.or(a.key),
            }),
        }
    }

    /// Apply env var overrides on top of this config.
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(val) = std::env::var("NKL_PORT") {
            if let Ok(port) = val.parse() {
                self.proxy.get_or_insert_with(Default::default).port = Some(port);
            }
        }
        if let Ok(val) = std::env::var("NKL_HTTPS") {
            let https = matches!(val.as_str(), "1" | "true" | "yes");
            self.proxy.get_or_insert_with(Default::default).https = Some(https);
        }
        if let Ok(val) = std::env::var("NKL_DOMAINS") {
            let domains: Vec<String> = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !domains.is_empty() {
                self.proxy.get_or_insert_with(Default::default).domains = Some(domains);
            }
        }
        if let Ok(val) = std::env::var("NKL_STATE_DIR") {
            self.paths.get_or_insert_with(Default::default).state_dir = Some(val);
        }
        self
    }

    /// Resolve into a Config with defaults applied.
    pub fn resolve(self) -> Config {
        let defaults = Config::default();

        Config {
            proxy_port: self
                .proxy
                .as_ref()
                .and_then(|p| p.port)
                .unwrap_or(defaults.proxy_port),
            proxy_https: self
                .proxy
                .as_ref()
                .and_then(|p| p.https)
                .unwrap_or(defaults.proxy_https),
            max_hops: self
                .proxy
                .as_ref()
                .and_then(|p| p.max_hops)
                .unwrap_or(defaults.max_hops),
            domains: self
                .proxy
                .as_ref()
                .and_then(|p| p.domains.clone())
                .unwrap_or(defaults.domains),
            app_port: self.app.as_ref().and_then(|a| a.port),
            app_port_range: (
                self.app
                    .as_ref()
                    .and_then(|a| a.port_range_start)
                    .unwrap_or(defaults.app_port_range.0),
                self.app
                    .as_ref()
                    .and_then(|a| a.port_range_end)
                    .unwrap_or(defaults.app_port_range.1),
            ),
            app_force: self
                .app
                .as_ref()
                .and_then(|a| a.force)
                .unwrap_or(defaults.app_force),
            ready_timeout: self
                .app
                .as_ref()
                .and_then(|a| a.ready_timeout)
                .unwrap_or(defaults.ready_timeout),
            hosts_auto_sync: self
                .hosts
                .as_ref()
                .and_then(|h| h.auto_sync)
                .unwrap_or(defaults.hosts_auto_sync),
            state_dir: self
                .paths
                .as_ref()
                .and_then(|p| p.state_dir.as_ref())
                .map(PathBuf::from),
            tls_cert: self
                .tls
                .as_ref()
                .and_then(|t| t.cert.as_ref())
                .map(PathBuf::from),
            tls_key: self
                .tls
                .as_ref()
                .and_then(|t| t.key.as_ref())
                .map(PathBuf::from),
        }
    }
}

fn merge_opt<T>(a: Option<T>, b: Option<T>, f: impl FnOnce(T, T) -> T) -> Option<T> {
    match (a, b) {
        (Some(a), Some(b)) => Some(f(a, b)),
        (None, b) => b,
        (a, None) => a,
    }
}

/// Config file search locations (lowest to highest priority).
fn config_file_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    paths.push(PathBuf::from("/etc/nkl/config.toml"));
    paths.push(dirs_or_home().join(".nkl").join("config.toml"));
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(project_config) = find_project_config(&cwd) {
            paths.push(project_config);
        }
    }
    paths
}

/// Walk up from `start` looking for `nkl.toml`.
fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut dir = start;
    loop {
        let candidate = dir.join("nkl.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

/// Load a single TOML config file, returning None if missing or unparseable.
fn load_config_file(path: &Path) -> Option<RawConfig> {
    let content = fs::read_to_string(path).ok()?;
    match toml::from_str(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            tracing::warn!("failed to parse config {}: {}", path.display(), e);
            None
        }
    }
}

/// Load the fully resolved config by merging all layers.
pub fn load_config() -> Config {
    let mut merged = RawConfig::default();
    for path in config_file_paths() {
        if let Some(raw) = load_config_file(&path) {
            tracing::debug!("loaded config from {}", path.display());
            merged = merged.merge(raw);
        }
    }
    merged = merged.with_env_overrides();
    merged.resolve()
}

impl Config {
    pub fn resolve_state_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.state_dir {
            return dir.clone();
        }
        if self.proxy_port < PRIVILEGED_PORT_THRESHOLD {
            PathBuf::from("/tmp/nkl")
        } else {
            dirs_or_home().join(".nkl")
        }
    }
}

fn dirs_or_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Print current resolved config to stdout.
pub fn print_config() {
    let config = load_config();
    println!("Config");
    println!("  proxy.port:       {}", config.proxy_port);
    println!("  proxy.https:      {}", config.proxy_https);
    println!("  proxy.max_hops:   {}", config.max_hops);
    println!("  proxy.domains:    {}", config.domains.join(", "));
    if let Some(port) = config.app_port {
        println!("  app.port:         {}", port);
    }
    println!(
        "  app.port_range:   {}-{}",
        config.app_port_range.0, config.app_port_range.1
    );
    println!("  app.force:        {}", config.app_force);
    println!("  app.ready_timeout: {}s", config.ready_timeout);
    println!("  hosts.auto_sync:  {}", config.hosts_auto_sync);
    println!(
        "  state_dir:        {}",
        config.resolve_state_dir().display()
    );
    if let Some(ref cert) = config.tls_cert {
        println!("  tls.cert:         {}", cert.display());
    }
    if let Some(ref key) = config.tls_key {
        println!("  tls.key:          {}", key.display());
    }

    println!();
    println!("Sources (lowest to highest priority):");
    for path in config_file_paths() {
        let exists = path.is_file();
        let marker = if exists { "+" } else { "-" };
        println!("  [{}] {}", marker, path.display());
    }
    println!("  [*] environment variables");
    println!("  [*] CLI flags");
}

#[cfg(test)]
mod tests;

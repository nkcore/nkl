use std::ffi::CString;
use std::path::{Path, PathBuf};

use nix::libc;

/// Default proxy port.
pub const DEFAULT_PORT: u16 = 1355;

/// Threshold below which ports require elevated privileges.
pub const PRIVILEGED_PORT_THRESHOLD: u16 = 1024;

/// Resolve the state directory based on the proxy port.
///
/// - Port < 1024: `/tmp/nkl` (shared between root and user)
/// - Port >= 1024: `~/.nkl` (user-scoped)
pub fn resolve_state_dir(port: u16) -> PathBuf {
    if let Ok(dir) = std::env::var("NKL_STATE_DIR") {
        return PathBuf::from(dir);
    }

    if port < PRIVILEGED_PORT_THRESHOLD {
        PathBuf::from("/tmp/nkl")
    } else {
        dirs_or_home().join(".nkl")
    }
}

fn dirs_or_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Sanitize a string for use as a .localhost hostname label.
pub fn sanitize_for_hostname(name: &str) -> String {
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens and trim
    let collapsed = sanitized
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    truncate_label(&collapsed)
}

/// Truncate a DNS label to 63 characters (RFC 1035).
fn truncate_label(label: &str) -> String {
    if label.len() <= 63 {
        return label.to_string();
    }

    use std::fmt::Write;
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        label.hash(&mut hasher);
        let h = hasher.finish();
        let mut buf = String::new();
        write!(buf, "{:06x}", h & 0xFFFFFF).unwrap();
        buf
    };

    let max_prefix = 63 - 7; // "-" + 6-char hash
    let prefix = &label[..max_prefix].trim_end_matches('-');
    format!("{}-{}", prefix, hash)
}

/// Parse and normalize a hostname, appending the first configured domain if needed.
///
/// `domains` is the list of allowed domain suffixes (e.g. `["localhost", "dev.local"]`).
/// If the input already ends with one of the domains, it is accepted as-is.
/// Otherwise, the first domain in the list is appended as suffix.
pub fn parse_hostname(input: &str, domains: &[String]) -> Result<String, String> {
    let hostname = input
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .to_lowercase();

    if hostname.is_empty() {
        return Err("Hostname cannot be empty".to_string());
    }

    let default_domain = domains.first().map(|s| s.as_str()).unwrap_or("localhost");

    // Check if hostname already ends with any configured domain
    let has_domain = domains
        .iter()
        .any(|d| hostname == *d || hostname.ends_with(&format!(".{}", d)));

    let hostname = if has_domain {
        hostname
    } else {
        format!("{}.{}", hostname, default_domain)
    };

    // Extract the name part (before the domain suffix) for validation
    let name = domains
        .iter()
        .find_map(|d| hostname.strip_suffix(&format!(".{}", d)))
        .or_else(|| {
            domains
                .iter()
                .find_map(|d| if hostname == *d { Some("") } else { None })
        })
        .unwrap_or(&hostname);

    if name.is_empty() {
        return Err("Hostname cannot be empty".to_string());
    }

    if name.contains("..") {
        return Err(format!(
            "Invalid hostname \"{}\": consecutive dots are not allowed",
            name
        ));
    }

    Ok(hostname)
}

/// Extract the hostname prefix (label before the domain suffix).
///
/// For example, given hostname "myapp.localhost" and domains ["localhost", "dev.local"],
/// returns "myapp".
pub fn extract_hostname_prefix(hostname: &str, domains: &[String]) -> String {
    for domain in domains {
        if let Some(prefix) = hostname.strip_suffix(&format!(".{}", domain)) {
            return prefix.to_string();
        }
    }
    hostname.to_string()
}

/// Generate URLs for all configured domains from a hostname prefix.
///
/// Given a prefix like "myapp" and domains ["localhost", "dev.local"],
/// returns URLs for "myapp.localhost" and "myapp.dev.local".
pub fn format_urls(
    hostname_prefix: &str,
    domains: &[String],
    proxy_port: u16,
    tls: bool,
) -> Vec<String> {
    domains
        .iter()
        .map(|domain| {
            let full_hostname = format!("{}.{}", hostname_prefix, domain);
            format_url(&full_hostname, proxy_port, tls)
        })
        .collect()
}

/// Format a .localhost URL.
pub fn format_url(hostname: &str, proxy_port: u16, tls: bool) -> String {
    let proto = if tls { "https" } else { "http" };
    let default_port = if tls { 443 } else { 80 };
    if proxy_port == default_port {
        format!("{}://{}", proto, hostname)
    } else {
        format!("{}://{}:{}", proto, hostname, proxy_port)
    }
}

/// Detect sudo environment from SUDO_UID and SUDO_GID env vars.
///
/// Returns `Some((uid, gid))` if both are present and valid, `None` otherwise.
fn detect_sudo_ids() -> Option<(libc::uid_t, libc::gid_t)> {
    let uid_str = std::env::var("SUDO_UID").ok()?;
    let gid_str = std::env::var("SUDO_GID").ok()?;
    let uid: libc::uid_t = uid_str.parse().ok()?;
    let gid: libc::gid_t = gid_str.parse().ok()?;
    Some((uid, gid))
}

/// Fix file/directory ownership when running under sudo.
///
/// If SUDO_UID and SUDO_GID environment variables are set, chown the given
/// path to the real (non-root) user. If not running under sudo, this is a
/// no-op.
///
/// Errors are logged as warnings but never propagated -- ownership fixup is
/// best-effort and must not break the caller.
pub fn fix_ownership(path: &Path) {
    let Some((uid, gid)) = detect_sudo_ids() else {
        return;
    };

    let c_path = match CString::new(path.as_os_str().as_encoded_bytes()) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("fix_ownership: invalid path {:?}: {}", path, e);
            return;
        }
    };

    // SAFETY: c_path is a valid null-terminated C string, and chown is a
    // standard POSIX call that does not violate memory safety.
    let ret = unsafe { libc::chown(c_path.as_ptr(), uid, gid) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        tracing::warn!(
            "fix_ownership: chown {:?} to {}:{} failed: {}",
            path,
            uid,
            gid,
            err
        );
    }
}

#[cfg(test)]
mod tests;

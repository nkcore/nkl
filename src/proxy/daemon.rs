use std::fs;
use std::path::Path;

use http_body_util::Full;
use hyper::Request;
use hyper::body::Bytes;

use crate::config::Config;

use super::NKL_HEADER;
use super::server::{cleanup_lifecycle_files, start_proxy};

// ---------------------------------------------------------------------------
// Daemon
// ---------------------------------------------------------------------------

/// Daemonize the current process and start the proxy.
pub fn daemonize_and_start_proxy(config: &Config) -> anyhow::Result<()> {
    let state_dir = config.resolve_state_dir();
    fs::create_dir_all(&state_dir)?;

    let log_path = state_dir.join("proxy.log");
    let log_file = fs::File::create(&log_path)?;
    let log_err = log_file.try_clone()?;
    let pid_file = state_dir.join("proxy.pid");

    let daemonize = daemonize::Daemonize::new()
        .pid_file(&pid_file)
        .chown_pid_file(true)
        .working_directory("/")
        .stdout(log_file)
        .stderr(log_err);

    match daemonize.start() {
        Ok(()) => {
            let log_writer = fs::File::options()
                .append(true)
                .open(state_dir.join("proxy.log"))
                .unwrap_or_else(|_| fs::File::create(state_dir.join("proxy.log")).unwrap());
            let _ = tracing_subscriber::fmt()
                .with_writer(std::sync::Mutex::new(log_writer))
                .with_ansi(false)
                .with_target(true)
                .with_level(true)
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::INFO.into()),
                )
                .try_init();

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(start_proxy(config))?;
            Ok(())
        }
        Err(e) => anyhow::bail!("failed to daemonize: {}", e),
    }
}

// ---------------------------------------------------------------------------
// Proxy status / control
// ---------------------------------------------------------------------------

/// Check if a nkl proxy is running by sending an HTTP HEAD request.
pub async fn is_proxy_running(port: u16) -> bool {
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();
    let uri = format!("http://127.0.0.1:{}/", port);
    let req = match Request::builder()
        .method("HEAD")
        .uri(&uri)
        .body(Full::new(Bytes::new()))
    {
        Ok(r) => r,
        Err(_) => return false,
    };
    match client.request(req).await {
        Ok(resp) => resp.headers().contains_key(NKL_HEADER),
        Err(_) => false,
    }
}

/// Wait for the proxy to become ready, polling up to `max_attempts` times.
pub async fn wait_for_proxy(port: u16, max_attempts: u32, interval_ms: u64) -> bool {
    for _ in 0..max_attempts {
        if is_proxy_running(port).await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
    }
    false
}

/// Stop a running proxy by reading PID from state dir and sending SIGTERM.
pub fn stop_proxy(state_dir: &Path) -> anyhow::Result<()> {
    let pid_path = state_dir.join("proxy.pid");
    if !pid_path.exists() {
        anyhow::bail!("proxy is not running (no PID file found)");
    }

    let pid_str = fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid PID in {}", pid_path.display()))?;

    // Check if process is alive
    let nix_pid = nix::unistd::Pid::from_raw(pid);
    let alive = nix::sys::signal::kill(nix_pid, None).is_ok();
    if !alive {
        cleanup_lifecycle_files(state_dir);
        anyhow::bail!(
            "proxy process {} is not running (stale PID file cleaned up)",
            pid
        );
    }

    // Send SIGTERM
    if let Err(e) = nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGTERM) {
        if e == nix::errno::Errno::EPERM {
            anyhow::bail!(
                "permission denied: cannot stop proxy (PID {}). Try: sudo nkl proxy stop",
                pid
            );
        }
        anyhow::bail!("failed to stop proxy (PID {}): {}", pid, e);
    }

    // Wait briefly for process to exit, then clean up
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let still_alive = nix::sys::signal::kill(nix_pid, None).is_ok();
        if !still_alive {
            cleanup_lifecycle_files(state_dir);
            tracing::info!("proxy stopped (PID {})", pid);
            return Ok(());
        }
    }

    cleanup_lifecycle_files(state_dir);
    tracing::warn!("proxy (PID {}) may still be running", pid);
    Ok(())
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

/// Show proxy daemon logs from state_dir/proxy.log.
pub async fn show_logs(
    state_dir: &Path,
    follow: bool,
    tail_lines: Option<usize>,
) -> anyhow::Result<()> {
    let log_path = state_dir.join("proxy.log");
    if !log_path.exists() {
        anyhow::bail!(
            "no log file found at {}. Is the proxy running in daemon mode?",
            log_path.display()
        );
    }

    let content = fs::read_to_string(&log_path)?;

    let output = match tail_lines {
        Some(n) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(n);
            lines[start..].join("\n")
        }
        None => content.clone(),
    };

    if !output.is_empty() {
        print!("{}", output);
        if !output.ends_with('\n') {
            println!();
        }
    }

    if !follow {
        return Ok(());
    }

    // Follow mode: poll for new content every 200ms
    use std::io::Write;
    let mut pos = content.len() as u64;

    loop {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let metadata = match fs::metadata(&log_path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let file_len = metadata.len();

        if file_len < pos {
            pos = 0;
        }

        if file_len > pos {
            let mut file = fs::File::open(&log_path)?;
            use std::io::{Read, Seek, SeekFrom};
            file.seek(SeekFrom::Start(pos))?;
            let mut buf = vec![0u8; (file_len - pos) as usize];
            file.read_exact(&mut buf)?;
            let text = String::from_utf8_lossy(&buf);
            print!("{}", text);
            std::io::stdout().flush()?;
            pos = file_len;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_proxy_stale_pid_file() {
        let dir = tempfile::tempdir().unwrap();
        let state_dir = dir.path();

        fs::write(state_dir.join("proxy.pid"), "999999999").unwrap();
        fs::write(state_dir.join("proxy.port"), "1355").unwrap();

        let result = stop_proxy(state_dir);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not running"));

        assert!(!state_dir.join("proxy.pid").exists());
        assert!(!state_dir.join("proxy.port").exists());
    }

    #[test]
    fn test_stop_proxy_no_pid_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = stop_proxy(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not running"));
    }

    #[tokio::test]
    async fn test_show_logs_no_log_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = show_logs(tmp.path(), false, None).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("no log file found"),
            "expected 'no log file found' error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_show_logs_reads_full_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_path = tmp.path().join("proxy.log");
        fs::write(&log_path, "line1\nline2\nline3\n").unwrap();

        let result = show_logs(tmp.path(), false, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_logs_with_tail_lines() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_path = tmp.path().join("proxy.log");
        fs::write(&log_path, "line1\nline2\nline3\nline4\nline5\n").unwrap();

        let result = show_logs(tmp.path(), false, Some(2)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_logs_tail_more_than_available() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_path = tmp.path().join("proxy.log");
        fs::write(&log_path, "line1\nline2\n").unwrap();

        let result = show_logs(tmp.path(), false, Some(100)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_show_logs_empty_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log_path = tmp.path().join("proxy.log");
        fs::write(&log_path, "").unwrap();

        let result = show_logs(tmp.path(), false, None).await;
        assert!(result.is_ok());
    }
}

use super::*;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_nkl_env<F: FnOnce()>(value: Option<&str>, f: F) {
    let _guard = ENV_LOCK.lock().unwrap();
    let saved = std::env::var("NKL").ok();

    unsafe {
        match value {
            Some(v) => std::env::set_var("NKL", v),
            None => std::env::remove_var("NKL"),
        }
    }

    f();

    unsafe {
        match saved {
            Some(v) => std::env::set_var("NKL", v),
            None => std::env::remove_var("NKL"),
        }
    }
}

#[test]
fn test_propagate_exit_status_success() {
    let status = std::process::Command::new("true").status().unwrap();
    propagate_exit_status(status);
}

#[test]
fn test_print_connection_info_does_not_panic() {
    let config = Config {
        proxy_port: 1355,
        proxy_https: false,
        domains: vec!["localhost".to_string(), "dev.local".to_string()],
        ..Config::default()
    };
    let cmd = vec!["npm".to_string(), "run".to_string(), "dev".to_string()];
    print_connection_info(&config, &cmd, 3000, 12345, "myapp.localhost");
}

#[test]
fn test_is_nkl_disabled_zero() {
    with_nkl_env(Some("0"), || {
        assert!(is_nkl_disabled());
    });
}

#[test]
fn test_is_nkl_disabled_skip() {
    with_nkl_env(Some("skip"), || {
        assert!(is_nkl_disabled());
    });
}

#[test]
fn test_is_nkl_disabled_skip_uppercase() {
    with_nkl_env(Some("SKIP"), || {
        assert!(is_nkl_disabled());
    });
}

#[test]
fn test_is_nkl_disabled_other_value() {
    with_nkl_env(Some("1"), || {
        assert!(!is_nkl_disabled());
    });
}

#[test]
fn test_is_nkl_disabled_unset() {
    with_nkl_env(None, || {
        assert!(!is_nkl_disabled());
    });
}

#[test]
fn test_is_nkl_disabled_empty() {
    with_nkl_env(Some(""), || {
        assert!(!is_nkl_disabled());
    });
}

#[test]
fn test_is_nkl_disabled_whitespace() {
    with_nkl_env(Some(" 0 "), || {
        assert!(is_nkl_disabled());
    });
}

#[tokio::test]
async fn test_run_direct_executes_command() {
    let config = Config::default();
    let cmd = vec!["true".to_string()];
    let result = run_direct(&config, &cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_direct_uses_configured_port() {
    let config = Config {
        app_port: Some(4567),
        ..Config::default()
    };
    let cmd = vec!["echo".to_string(), "$PORT".to_string()];
    let result = run_direct(&config, &cmd).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_direct_default_port_zero() {
    let config = Config::default();
    let cmd = vec![
        "test".to_string(),
        "$PORT".to_string(),
        "=".to_string(),
        "0".to_string(),
    ];
    let result = run_direct(&config, &cmd).await;
    assert!(result.is_ok());
}

#[test]
fn test_name_override_resolves_hostname() {
    let domains = vec!["localhost".to_string()];
    let hostname = parse_hostname("myapp", &domains).unwrap();
    assert_eq!(hostname, "myapp.localhost");
}

#[test]
fn test_name_override_normalizes_input() {
    let domains = vec!["localhost".to_string()];
    let hostname = parse_hostname("MyApp", &domains).unwrap();
    assert_eq!(hostname, "myapp.localhost");
}

#[test]
fn test_name_override_with_full_hostname() {
    let domains = vec!["localhost".to_string()];
    let hostname = parse_hostname("myapp.localhost", &domains).unwrap();
    assert_eq!(hostname, "myapp.localhost");
}

#[test]
fn test_name_override_custom_domain() {
    let domains = vec!["dev.local".to_string(), "localhost".to_string()];
    let hostname = parse_hostname("myapp", &domains).unwrap();
    assert_eq!(hostname, "myapp.dev.local");
}

#[test]
fn test_name_override_empty_rejected() {
    let domains = vec!["localhost".to_string()];
    let result = parse_hostname("", &domains);
    assert!(result.is_err());
}

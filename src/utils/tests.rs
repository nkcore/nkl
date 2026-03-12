use super::*;
use std::path::PathBuf;

#[test]
fn test_sanitize_for_hostname() {
    assert_eq!(sanitize_for_hostname("MyApp"), "myapp");
    assert_eq!(sanitize_for_hostname("my_app"), "my-app");
    assert_eq!(sanitize_for_hostname("@scope/pkg"), "scope-pkg");
    assert_eq!(sanitize_for_hostname("--hello--"), "hello");
}

#[test]
fn test_parse_hostname() {
    let domains = vec!["localhost".to_string()];
    assert_eq!(
        parse_hostname("myapp", &domains).unwrap(),
        "myapp.localhost"
    );
    assert_eq!(
        parse_hostname("http://myapp.localhost", &domains).unwrap(),
        "myapp.localhost"
    );
    assert!(parse_hostname("", &domains).is_err());
}

#[test]
fn test_parse_hostname_custom_domains() {
    let domains = vec!["dev.local".to_string(), "localhost".to_string()];

    // Default domain is first in list
    assert_eq!(
        parse_hostname("myapp", &domains).unwrap(),
        "myapp.dev.local"
    );

    // Recognizes both configured domains
    assert_eq!(
        parse_hostname("myapp.dev.local", &domains).unwrap(),
        "myapp.dev.local"
    );
    assert_eq!(
        parse_hostname("myapp.localhost", &domains).unwrap(),
        "myapp.localhost"
    );

    // Strips protocol
    assert_eq!(
        parse_hostname("http://api.dev.local", &domains).unwrap(),
        "api.dev.local"
    );
}

#[test]
fn test_parse_hostname_single_custom_domain() {
    let domains = vec!["test".to_string()];
    assert_eq!(parse_hostname("myapp", &domains).unwrap(), "myapp.test");
    assert_eq!(
        parse_hostname("myapp.test", &domains).unwrap(),
        "myapp.test"
    );
}

#[test]
fn test_format_url() {
    assert_eq!(
        format_url("myapp.localhost", 1355, false),
        "http://myapp.localhost:1355"
    );
    assert_eq!(
        format_url("myapp.localhost", 80, false),
        "http://myapp.localhost"
    );
    assert_eq!(
        format_url("myapp.localhost", 443, true),
        "https://myapp.localhost"
    );
}

#[test]
fn test_truncate_label() {
    let short = "myapp";
    assert_eq!(truncate_label(short), "myapp");

    let long = "a".repeat(100);
    let truncated = truncate_label(&long);
    assert!(truncated.len() <= 63);
}

#[test]
fn test_extract_hostname_prefix() {
    let domains = vec!["localhost".to_string(), "dev.local".to_string()];
    assert_eq!(
        extract_hostname_prefix("myapp.localhost", &domains),
        "myapp"
    );
    assert_eq!(
        extract_hostname_prefix("myapp.dev.local", &domains),
        "myapp"
    );
    assert_eq!(
        extract_hostname_prefix("myapp.unknown", &domains),
        "myapp.unknown"
    );
}

#[test]
fn test_format_urls_single_domain() {
    let domains = vec!["localhost".to_string()];
    let urls = format_urls("myapp", &domains, 1355, false);
    assert_eq!(urls, vec!["http://myapp.localhost:1355"]);
}

#[test]
fn test_format_urls_multiple_domains() {
    let domains = vec!["localhost".to_string(), "dev.local".to_string()];
    let urls = format_urls("myapp", &domains, 1355, false);
    assert_eq!(
        urls,
        vec!["http://myapp.localhost:1355", "http://myapp.dev.local:1355",]
    );
}

#[test]
fn test_format_urls_default_port() {
    let domains = vec!["localhost".to_string()];
    let urls = format_urls("myapp", &domains, 80, false);
    assert_eq!(urls, vec!["http://myapp.localhost"]);
}

#[test]
fn test_format_urls_tls() {
    let domains = vec!["localhost".to_string()];
    let urls = format_urls("myapp", &domains, 443, true);
    assert_eq!(urls, vec!["https://myapp.localhost"]);
}

#[test]
fn test_resolve_state_dir() {
    if std::env::var("NKL_STATE_DIR").is_ok() {
        // If env var is set, resolve_state_dir returns that value
        let dir = resolve_state_dir(80);
        assert_eq!(dir, PathBuf::from(std::env::var("NKL_STATE_DIR").unwrap()));
    } else {
        let dir = resolve_state_dir(80);
        assert_eq!(dir, PathBuf::from("/tmp/nkl"));

        let dir = resolve_state_dir(1355);
        assert!(dir.to_str().unwrap().ends_with(".nkl"));
    }
}

/// All env-var-dependent sudo detection tests are combined into a single
/// test function to avoid races from parallel test threads mutating the
/// shared process environment.
#[test]
fn test_sudo_detection_and_fix_ownership() {
    // SAFETY: env manipulation in tests. Combined into one test to avoid
    // parallel mutation of the process environment.

    // --- detect_sudo_ids: not set ---
    unsafe {
        std::env::remove_var("SUDO_UID");
        std::env::remove_var("SUDO_GID");
    }
    assert!(
        detect_sudo_ids().is_none(),
        "should be None when env vars not set"
    );

    // --- fix_ownership: no-op without sudo ---
    fix_ownership(Path::new("/tmp/nonexistent_test_file"));

    // --- detect_sudo_ids: both set ---
    unsafe {
        std::env::set_var("SUDO_UID", "1000");
        std::env::set_var("SUDO_GID", "1000");
    }
    assert_eq!(
        detect_sudo_ids(),
        Some((1000, 1000)),
        "should parse valid uid/gid"
    );

    // --- fix_ownership: graceful on nonexistent path ---
    fix_ownership(Path::new("/tmp/nkl_test_nonexistent_path_abc123"));

    // --- detect_sudo_ids: only uid set ---
    unsafe {
        std::env::remove_var("SUDO_GID");
    }
    assert!(
        detect_sudo_ids().is_none(),
        "should be None when SUDO_GID missing"
    );

    // --- detect_sudo_ids: invalid values ---
    unsafe {
        std::env::set_var("SUDO_UID", "not_a_number");
        std::env::set_var("SUDO_GID", "1000");
    }
    assert!(
        detect_sudo_ids().is_none(),
        "should be None for non-numeric uid"
    );

    // --- cleanup ---
    unsafe {
        std::env::remove_var("SUDO_UID");
        std::env::remove_var("SUDO_GID");
    }
}

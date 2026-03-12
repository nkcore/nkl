use super::*;

#[test]
fn test_default_config() {
    let config = Config::default();
    assert_eq!(config.proxy_port, 1355);
    assert!(!config.proxy_https);
    assert_eq!(config.max_hops, 5);
    assert_eq!(config.domains, vec!["localhost".to_string()]);
    assert_eq!(config.app_port_range, (3000, 9999));
    assert!(!config.app_force);
    assert!(!config.hosts_auto_sync);
    assert!(config.state_dir.is_none());
}

#[test]
fn test_raw_config_resolve_defaults() {
    let raw = RawConfig::default();
    let config = raw.resolve();
    assert_eq!(config.proxy_port, 1355);
    assert!(!config.proxy_https);
}

#[test]
fn test_raw_config_resolve_overrides() {
    let raw = RawConfig {
        proxy: Some(RawProxy {
            port: Some(8080),
            https: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };
    let config = raw.resolve();
    assert_eq!(config.proxy_port, 8080);
    assert!(config.proxy_https);
    assert_eq!(config.max_hops, 5); // default
}

#[test]
fn test_merge_both_present() {
    let base = RawConfig {
        proxy: Some(RawProxy {
            port: Some(1355),
            https: Some(false),
            max_hops: Some(3),
            ..Default::default()
        }),
        ..Default::default()
    };
    let overlay = RawConfig {
        proxy: Some(RawProxy {
            port: Some(8080),
            ..Default::default()
        }),
        app: Some(RawApp {
            force: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };
    let merged = base.merge(overlay).resolve();
    assert_eq!(merged.proxy_port, 8080); // overlay wins
    assert!(!merged.proxy_https); // base kept
    assert_eq!(merged.max_hops, 3); // base kept
    assert!(merged.app_force); // overlay added
}

#[test]
fn test_merge_only_base() {
    let base = RawConfig {
        proxy: Some(RawProxy {
            port: Some(9090),
            ..Default::default()
        }),
        ..Default::default()
    };
    let merged = base.merge(RawConfig::default()).resolve();
    assert_eq!(merged.proxy_port, 9090);
    assert_eq!(merged.domains, vec!["localhost".to_string()]); // default preserved
}

#[test]
fn test_merge_only_overlay() {
    let overlay = RawConfig {
        hosts: Some(RawHosts {
            auto_sync: Some(true),
        }),
        ..Default::default()
    };
    let merged = RawConfig::default().merge(overlay).resolve();
    assert!(merged.hosts_auto_sync);
}

#[test]
fn test_load_config_file_missing() {
    let result = load_config_file(Path::new("/nonexistent/config.toml"));
    assert!(result.is_none());
}

#[test]
fn test_load_config_file_valid() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[proxy]
port = 4000
https = true

[app]
force = true
"#,
    )
    .unwrap();

    let raw = load_config_file(&config_path).unwrap();
    let config = raw.resolve();
    assert_eq!(config.proxy_port, 4000);
    assert!(config.proxy_https);
    assert!(config.app_force);
}

#[test]
fn test_load_config_file_invalid_toml() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(&config_path, "not valid [[[ toml").unwrap();

    let result = load_config_file(&config_path);
    assert!(result.is_none());
}

#[test]
fn test_find_project_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sub = tmp.path().join("a").join("b");
    fs::create_dir_all(&sub).unwrap();
    fs::write(tmp.path().join("nkl.toml"), "[proxy]\nport = 7777\n").unwrap();

    let found = find_project_config(&sub);
    assert_eq!(found.unwrap(), tmp.path().join("nkl.toml"));
}

#[test]
fn test_find_project_config_not_found() {
    let tmp = tempfile::TempDir::new().unwrap();
    let found = find_project_config(tmp.path());
    assert!(found.is_none());
}

#[test]
fn test_config_resolve_state_dir_explicit() {
    let config = Config {
        state_dir: Some(PathBuf::from("/custom/state")),
        ..Default::default()
    };
    assert_eq!(config.resolve_state_dir(), PathBuf::from("/custom/state"));
}

#[test]
fn test_custom_domains_config() {
    let raw = RawConfig {
        proxy: Some(RawProxy {
            domains: Some(vec!["dev.local".to_string(), "test".to_string()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let config = raw.resolve();
    assert_eq!(
        config.domains,
        vec!["dev.local".to_string(), "test".to_string()]
    );
}

#[test]
fn test_domains_merge_overlay_wins() {
    let base = RawConfig {
        proxy: Some(RawProxy {
            domains: Some(vec!["localhost".to_string()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let overlay = RawConfig {
        proxy: Some(RawProxy {
            domains: Some(vec!["dev.local".to_string(), "localhost".to_string()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let config = base.merge(overlay).resolve();
    assert_eq!(
        config.domains,
        vec!["dev.local".to_string(), "localhost".to_string()]
    );
}

#[test]
fn test_load_config_file_with_domains() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
[proxy]
port = 1355
domains = ["dev.local", "localhost", "test"]
"#,
    )
    .unwrap();

    let raw = load_config_file(&config_path).unwrap();
    let config = raw.resolve();
    assert_eq!(
        config.domains,
        vec![
            "dev.local".to_string(),
            "localhost".to_string(),
            "test".to_string(),
        ]
    );
}

#[test]
fn test_config_resolve_state_dir_privileged_port() {
    let config = Config {
        proxy_port: 80,
        state_dir: None,
        ..Default::default()
    };
    assert_eq!(config.resolve_state_dir(), PathBuf::from("/tmp/nkl"));
}

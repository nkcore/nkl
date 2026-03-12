use super::*;
use tempfile::TempDir;

#[test]
fn test_extract_managed_block_present() {
    let content = "\
127.0.0.1 localhost
# nkl-start
127.0.0.1 myapp.localhost
127.0.0.1 api.dev.local
# nkl-end
::1 localhost";

    let block = extract_managed_block(content).unwrap();
    assert_eq!(block.len(), 2);
    assert_eq!(block[0], "127.0.0.1 myapp.localhost");
    assert_eq!(block[1], "127.0.0.1 api.dev.local");
}

#[test]
fn test_extract_managed_block_absent() {
    let content = "127.0.0.1 localhost\n::1 localhost\n";
    assert!(extract_managed_block(content).is_none());
}

#[test]
fn test_extract_managed_block_empty() {
    let content = "\
# nkl-start
# nkl-end
";
    let block = extract_managed_block(content).unwrap();
    assert!(block.is_empty());
}

#[test]
fn test_extract_managed_block_unclosed() {
    let content = "\
# nkl-start
127.0.0.1 myapp.localhost
";
    assert!(extract_managed_block(content).is_none());
}

#[test]
fn test_remove_block() {
    let content = "\
127.0.0.1 localhost
# nkl-start
127.0.0.1 myapp.localhost
# nkl-end
::1 localhost
";
    let cleaned = remove_block(content);
    assert!(!cleaned.contains("nkl-start"));
    assert!(!cleaned.contains("myapp.localhost"));
    assert!(!cleaned.contains("nkl-end"));
    assert!(cleaned.contains("127.0.0.1 localhost"));
    assert!(cleaned.contains("::1 localhost"));
}

#[test]
fn test_remove_block_no_block() {
    let content = "127.0.0.1 localhost\n::1 localhost\n";
    let cleaned = remove_block(content);
    assert_eq!(cleaned, content);
}

#[test]
fn test_build_block() {
    let hostnames = vec!["myapp.localhost".to_string(), "api.dev.local".to_string()];
    let block = build_block(&hostnames);
    assert!(block.starts_with("# nkl-start\n"));
    assert!(block.contains("127.0.0.1 myapp.localhost\n"));
    assert!(block.contains("127.0.0.1 api.dev.local\n"));
    assert!(block.ends_with("# nkl-end"));
}

#[test]
fn test_build_block_empty() {
    let block = build_block(&[]);
    assert!(block.is_empty());
}

#[test]
fn test_sync_hosts_file_new() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(&hosts_path, "127.0.0.1 localhost\n").unwrap();

    let hostnames = vec!["myapp.localhost".to_string()];
    sync_hosts_file(&hostnames, &hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert!(content.contains("127.0.0.1 localhost"));
    assert!(content.contains("# nkl-start"));
    assert!(content.contains("127.0.0.1 myapp.localhost"));
    assert!(content.contains("# nkl-end"));
}

#[test]
fn test_sync_hosts_file_replace_existing() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(
        &hosts_path,
        "\
127.0.0.1 localhost
# nkl-start
127.0.0.1 old-app.localhost
# nkl-end
::1 localhost
",
    )
    .unwrap();

    let hostnames = vec!["new-app.localhost".to_string()];
    sync_hosts_file(&hostnames, &hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert!(content.contains("127.0.0.1 localhost"));
    assert!(content.contains("::1 localhost"));
    assert!(!content.contains("old-app.localhost"));
    assert!(content.contains("127.0.0.1 new-app.localhost"));
    assert_eq!(content.matches("# nkl-start").count(), 1);
    assert_eq!(content.matches("# nkl-end").count(), 1);
}

#[test]
fn test_sync_hosts_file_empty_hostnames() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(
        &hosts_path,
        "\
127.0.0.1 localhost
# nkl-start
127.0.0.1 myapp.localhost
# nkl-end
",
    )
    .unwrap();

    sync_hosts_file(&[], &hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert!(content.contains("127.0.0.1 localhost"));
    assert!(!content.contains("nkl-start"));
    assert!(!content.contains("nkl-end"));
}

#[test]
fn test_sync_hosts_file_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");

    let hostnames = vec!["myapp.localhost".to_string()];
    sync_hosts_file(&hostnames, &hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert!(content.contains("# nkl-start"));
    assert!(content.contains("127.0.0.1 myapp.localhost"));
}

#[test]
fn test_clean_hosts_file() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(
        &hosts_path,
        "\
127.0.0.1 localhost
# nkl-start
127.0.0.1 myapp.localhost
# nkl-end
::1 localhost
",
    )
    .unwrap();

    clean_hosts_file(&hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert!(content.contains("127.0.0.1 localhost"));
    assert!(content.contains("::1 localhost"));
    assert!(!content.contains("nkl-start"));
    assert!(!content.contains("myapp.localhost"));
}

#[test]
fn test_clean_hosts_file_no_block() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    let original = "127.0.0.1 localhost\n::1 localhost\n";
    fs::write(&hosts_path, original).unwrap();

    clean_hosts_file(&hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert_eq!(content, original);
}

#[test]
fn test_clean_hosts_file_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    clean_hosts_file(&hosts_path).unwrap();
}

#[test]
fn test_get_managed_hostnames() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(
        &hosts_path,
        "\
127.0.0.1 localhost
# nkl-start
127.0.0.1 myapp.localhost
127.0.0.1 api.dev.local
# nkl-end
",
    )
    .unwrap();

    let hostnames = get_managed_hostnames(&hosts_path).unwrap();
    assert_eq!(hostnames, vec!["myapp.localhost", "api.dev.local"]);
}

#[test]
fn test_get_managed_hostnames_no_block() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(&hosts_path, "127.0.0.1 localhost\n").unwrap();

    let hostnames = get_managed_hostnames(&hosts_path).unwrap();
    assert!(hostnames.is_empty());
}

#[test]
fn test_get_managed_hostnames_nonexistent() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");

    let hostnames = get_managed_hostnames(&hosts_path).unwrap();
    assert!(hostnames.is_empty());
}

#[test]
fn test_collect_hostnames_from_routes() {
    let routes = vec![
        crate::routes::RouteMapping {
            hostname: "b-app.localhost".to_string(),
            port: 3000,
            pid: 0,
            change_origin: false,
            path_prefix: "/".to_string(),
            strip_prefix: false,
        },
        crate::routes::RouteMapping {
            hostname: "a-app.localhost".to_string(),
            port: 3001,
            pid: 0,
            change_origin: false,
            path_prefix: "/".to_string(),
            strip_prefix: false,
        },
        crate::routes::RouteMapping {
            hostname: "b-app.localhost".to_string(),
            port: 3002,
            pid: 0,
            change_origin: false,
            path_prefix: "/api".to_string(),
            strip_prefix: false,
        },
    ];

    let hostnames = collect_hostnames_from_routes(&routes);
    assert_eq!(hostnames, vec!["a-app.localhost", "b-app.localhost"]);
}

#[test]
fn test_preserves_other_entries() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    let original = "\
127.0.0.1 localhost
192.168.1.100 myserver
# nkl-start
127.0.0.1 old.localhost
# nkl-end
10.0.0.1 another-host
";
    fs::write(&hosts_path, original).unwrap();

    let hostnames = vec!["new.localhost".to_string()];
    sync_hosts_file(&hostnames, &hosts_path).unwrap();

    let content = fs::read_to_string(&hosts_path).unwrap();
    assert!(content.contains("192.168.1.100 myserver"));
    assert!(content.contains("10.0.0.1 another-host"));
    assert!(content.contains("127.0.0.1 new.localhost"));
    assert!(!content.contains("old.localhost"));
}

#[test]
fn test_roundtrip_sync_then_read() {
    let tmp = TempDir::new().unwrap();
    let hosts_path = tmp.path().join("hosts");
    fs::write(&hosts_path, "127.0.0.1 localhost\n").unwrap();

    let hostnames = vec!["app1.localhost".to_string(), "app2.dev.local".to_string()];
    sync_hosts_file(&hostnames, &hosts_path).unwrap();

    let read_back = get_managed_hostnames(&hosts_path).unwrap();
    assert_eq!(read_back, hostnames);
}

use crate::config;

use super::{Cli, Commands, HostsAction, ProxyAction, apply_cli_overrides};

pub(super) async fn handle(cli: Cli) -> anyhow::Result<()> {
    let mut config = config::load_config();

    match cli.command {
        Commands::Run {
            cmd,
            name,
            https,
            app_port,
            force,
            change_origin,
        } => {
            if https {
                config.proxy_https = true;
            }
            if let Some(p) = app_port {
                config.app_port = Some(p);
            }
            if force {
                config.app_force = true;
            }
            crate::run::run_app(&config, &cmd, name.as_deref(), change_origin).await
        }
        Commands::Proxy { action } => match action {
            ProxyAction::Start {
                port,
                https,
                foreground,
                daemonize,
            } => {
                apply_cli_overrides(&mut config, port, https);

                if daemonize {
                    crate::proxy::daemonize_and_start_proxy(&config)
                } else if !foreground {
                    if crate::proxy::is_proxy_running(config.proxy_port).await {
                        println!("proxy is already running on port {}", config.proxy_port);
                        return Ok(());
                    }

                    let exe = std::env::current_exe()?;
                    let mut args = vec![
                        "proxy".to_string(),
                        "start".to_string(),
                        "--daemonize".to_string(),
                        "--port".to_string(),
                        config.proxy_port.to_string(),
                    ];
                    if config.proxy_https {
                        args.push("--https".to_string());
                    }

                    std::process::Command::new(exe)
                        .args(&args)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()?;

                    let state_dir = config.resolve_state_dir();
                    let log_path = state_dir.join("proxy.log");
                    if crate::proxy::wait_for_proxy(config.proxy_port, 20, 250).await {
                        println!("proxy started on http://localhost:{}", config.proxy_port);
                    } else {
                        anyhow::bail!(
                            "proxy failed to start. Check logs at {}",
                            log_path.display()
                        );
                    }
                    Ok(())
                } else {
                    crate::proxy::start_proxy(&config).await
                }
            }
            ProxyAction::Stop => {
                let state_dir = config.resolve_state_dir();
                crate::proxy::stop_proxy(&state_dir)
            }
            ProxyAction::Logs { follow, lines } => {
                let state_dir = config.resolve_state_dir();
                crate::proxy::show_logs(&state_dir, follow, lines).await
            }
        },
        Commands::Alias {
            name,
            port,
            remove,
            force,
            change_origin,
            path,
            strip_prefix,
        } => {
            let state_dir = config.resolve_state_dir();
            let store = crate::routes::RouteStore::new(state_dir);

            if remove {
                let name =
                    name.ok_or_else(|| anyhow::anyhow!("alias name is required for --remove"))?;
                let hostname = crate::utils::parse_hostname(&name, &config.domains)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                let path_filter = if path == "/" {
                    None
                } else {
                    Some(path.as_str())
                };
                store.remove_route(&hostname, path_filter)?;
                println!("removed alias: {}", hostname);
            } else {
                let name = name.ok_or_else(|| anyhow::anyhow!("alias name is required"))?;
                let port = port.ok_or_else(|| anyhow::anyhow!("target port is required"))?;
                let hostname = crate::utils::parse_hostname(&name, &config.domains)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                store.add_route(
                    &hostname,
                    port,
                    0,
                    force,
                    change_origin,
                    &path,
                    strip_prefix,
                )?;
                let url =
                    crate::utils::format_url(&hostname, config.proxy_port, config.proxy_https);
                let path_info = if path != "/" {
                    format!("{}", path)
                } else {
                    String::new()
                };
                println!("{}{} -> localhost:{}", url, path_info, port);
            }
            Ok(())
        }
        Commands::Get { name } => {
            let hostname = crate::utils::parse_hostname(&name, &config.domains)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let url = crate::utils::format_url(&hostname, config.proxy_port, config.proxy_https);
            println!("{}", url);
            Ok(())
        }
        Commands::List => {
            let state_dir = config.resolve_state_dir();
            let store = crate::routes::RouteStore::new(state_dir);
            let routes = store.load_routes()?;
            if routes.is_empty() {
                println!("No active routes.");
            } else {
                for route in &routes {
                    let url = crate::utils::format_url(
                        &route.hostname,
                        config.proxy_port,
                        config.proxy_https,
                    );
                    let path_info = if route.path_prefix != "/" {
                        route.path_prefix.as_str()
                    } else {
                        ""
                    };
                    let mut flags = Vec::new();
                    if route.change_origin {
                        flags.push("change_origin");
                    }
                    if route.strip_prefix && route.path_prefix != "/" {
                        flags.push("strip_prefix");
                    }
                    let flags_str = if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", flags.join(", "))
                    };
                    println!(
                        "  {}{} -> localhost:{}  (pid {}){}",
                        url, path_info, route.port, route.pid, flags_str
                    );
                }
            }
            Ok(())
        }
        Commands::Status => {
            crate::status::print_status();
            Ok(())
        }
        Commands::Config => {
            config::print_config();
            Ok(())
        }
        Commands::Trust => {
            let state_dir = config.resolve_state_dir();
            match crate::certs::ensure_certs(&state_dir) {
                Ok(paths) => {
                    println!("CA certificate: {}", paths.ca_cert.display());
                }
                Err(e) => {
                    anyhow::bail!("failed to generate certificates: {}", e);
                }
            }

            if crate::certs::is_ca_trusted(&state_dir) {
                println!("CA is already trusted by the system.");
                return Ok(());
            }

            println!("installing CA into system trust store...");
            match crate::certs::trust_ca(&state_dir)? {
                crate::certs::TrustResult::AlreadyTrusted => {
                    println!("CA is already trusted by the system.");
                }
                crate::certs::TrustResult::Installed => {
                    println!("CA installed successfully. HTTPS is now available.");
                }
                crate::certs::TrustResult::PermissionDenied(msg) => {
                    anyhow::bail!("{}", msg);
                }
                crate::certs::TrustResult::Failed(msg) => {
                    anyhow::bail!("failed to install CA: {}", msg);
                }
            }
            Ok(())
        }
        Commands::Hosts { action } => match action {
            HostsAction::Sync => {
                let state_dir = config.resolve_state_dir();
                let store = crate::routes::RouteStore::new(state_dir);
                let routes = store.load_routes()?;
                let hostnames = crate::hosts::collect_hostnames_from_routes(&routes);

                if hostnames.is_empty() {
                    println!("no routes registered, nothing to sync");
                    return Ok(());
                }

                let hosts_path = std::path::PathBuf::from("/etc/hosts");
                match crate::hosts::sync_hosts_file(&hostnames, &hosts_path) {
                    Ok(()) => {
                        println!("synced {} hostname(s) to /etc/hosts:", hostnames.len());
                        for h in &hostnames {
                            println!("  127.0.0.1 {}", h);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        anyhow::bail!(
                            "permission denied writing /etc/hosts. Try: sudo nkl hosts sync"
                        );
                    }
                    Err(e) => return Err(e.into()),
                }
                Ok(())
            }
            HostsAction::Clean => {
                let hosts_path = std::path::PathBuf::from("/etc/hosts");
                match crate::hosts::clean_hosts_file(&hosts_path) {
                    Ok(()) => {
                        println!("removed nkl entries from /etc/hosts");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        anyhow::bail!(
                            "permission denied writing /etc/hosts. Try: sudo nkl hosts clean"
                        );
                    }
                    Err(e) => return Err(e.into()),
                }
                Ok(())
            }
        },
    }
}

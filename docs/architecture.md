# NKL Architecture

> Updated: 2026-03-12

## How It Works

```text
Browser (myapp.localhost:1355)
    |
    v
Proxy server (port 1355)     <- nkl proxy start
    |
    |-> :4123 (myapp)        <- nkl run next dev
    +-> :4567 (api)          <- nkl alias api.localhost 4567
```

The proxy listens on a fixed port and routes requests by Host header to registered application ports. Route state is persisted in `routes.json` and protected by directory-based file locking.

## Module Structure

```text
src/
├── main.rs                  # Entry: tracing init + clap parse + cli.run()
├── error.rs                 # NKLError
├── npx_guard.rs             # Guard against accidental npx/dlx execution
├── routes.rs                # RouteStore: JSON persistence + mkdir-based file lock
├── status.rs                # Proxy + route status display
├── cli/
│   ├── mod.rs               # Cli struct + Commands enum (clap derive)
│   └── handler.rs           # Command dispatch
├── proxy/
│   ├── mod.rs               # Shared types, path helpers, re-exports
│   ├── server.rs            # TcpListener, route polling, accept loop, TLS
│   ├── handler.rs           # HTTP routing + forwarding logic
│   ├── daemon.rs            # Daemonization, health checks, stop/logs
│   ├── websocket.rs         # WebSocket upgrade handling
│   ├── handler_tests.rs
│   ├── websocket_tests.rs
│   └── websocket_unit_tests.rs
├── run/
│   ├── mod.rs               # Port alloc -> route register -> spawn -> cleanup
│   ├── framework.rs         # Framework detection + flag injection
│   └── tests.rs
├── discover/
│   ├── mod.rs               # Project name inference (package.json -> git -> cwd)
│   ├── worktree.rs          # Git worktree branch detection
│   └── worktree_tests.rs
├── config/
│   ├── mod.rs               # /etc/nkl + ~/.nkl + project + env merging
│   └── tests.rs
├── certs/
│   ├── mod.rs               # Cert validation, TLS server config
│   ├── generate.rs          # CA + server + host cert generation
│   ├── sni.rs               # SNI-based per-hostname cert resolver
│   └── trust.rs             # System CA trust store integration
├── hosts/
│   ├── mod.rs               # /etc/hosts file management
│   └── tests.rs
├── pages/
│   ├── mod.rs               # Branded HTML error pages
│   └── tests.rs
└── utils/
    ├── mod.rs               # Hostname parsing, URL formatting, state dir resolution
    └── tests.rs
```

## Key Types and Patterns

- `RouteCache`: `Arc<RwLock<Vec<RouteMapping>>>`
  - Shared in-memory route cache synchronized from `routes.json` by a background polling task.
- `RouteStore`
  - Route persistence layer using `mkdir` directory locking and an RAII `LockGuard`.
- `Config`
  - Layered configuration loaded from `/etc/nkl/config.toml` -> `~/.nkl/config.toml` -> `nkl.toml` -> `NKL_*` -> CLI overrides.
- `NKLError`
  - Core error type covering route conflicts, lock failures, hostname validation, and related cases.
- State directory
  - Privileged ports use `/tmp/nkl`
  - Non-privileged ports use `~/.nkl`
  - Overridable with `NKL_STATE_DIR`

## Request Flow

### `nkl run`

1. Infer or parse the hostname.
2. Ensure the proxy process is running.
3. Allocate an application port.
4. Write the route into `routes.json`.
5. Inject `PORT`, `HOST`, and `NKL_URL`.
6. Remove the route after the child process exits.

### Proxy request handling

1. Extract the hostname from the Host header or URI.
2. Check `x-nkl-hops` for loop detection.
3. Match the route by exact hostname and longest path prefix.
4. Add `X-Forwarded-*` headers and `X-NKL`.
5. Apply `change_origin` when requested.
6. Forward the request to the target application port.
7. Return branded HTML pages for 404, 502, and 508 cases.

### HTTPS

1. Ensure the CA and default server certificate exist at startup.
2. Generate and cache per-hostname certificates via SNI.
3. Support both HTTP/1.1 and HTTP/2.

## Runtime Interface

### Environment variables

- `NKL`
- `NKL_PORT`
- `NKL_APP_PORT`
- `NKL_HTTPS`
- `NKL_DOMAINS`
- `NKL_STATE_DIR`
- `NKL_SYNC_HOSTS`
- `NKL_URL`

### Paths

- Project config: `nkl.toml`
- System config: `/etc/nkl/config.toml`
- User config: `~/.nkl/config.toml`
- User state directory: `~/.nkl`
- System state directory: `/tmp/nkl`

### Proxy identity

- Header: `X-NKL`
- Hop header: `x-nkl-hops`
- Hosts markers: `# nkl-start` / `# nkl-end`

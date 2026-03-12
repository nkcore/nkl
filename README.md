# NKL

NKL replaces port numbers with stable, named local URLs.

It runs a local reverse proxy, assigns a hostname to each app, and forwards requests to the right local port.

## What It Does

- Runs local apps behind a fixed proxy port
- Maps app names to `*.localhost` URLs
- Persists active routes in a local state directory
- Supports static aliases for services such as Docker containers
- Supports HTTPS with a locally trusted CA
- Can manage `/etc/hosts` entries for additional local domains

## Install

Build and install from the repository root:

```bash
cargo install --path .
```

For development, you can also run it without installing:

```bash
cargo run -- --help
```

## Quick Start

Run an app through NKL:

```bash
nkl run npm run dev
```

NKL will:

- infer an app name from `package.json`, the Git root, or the current directory
- start the proxy automatically if needed
- choose a free app port when the command does not provide one
- expose the app at `http://<name>.localhost:1355`

If you want to set the app name explicitly:

```bash
nkl run --name web npm run dev
```

Get the generated URL from a script:

```bash
nkl get web
```

## Typical Workflows

Start the proxy explicitly:

```bash
nkl proxy start
```

Run the proxy in the foreground:

```bash
nkl proxy start --foreground
```

See active routes:

```bash
nkl list
```

Show proxy status:

```bash
nkl status
```

View daemon logs:

```bash
nkl proxy logs --follow
```

Stop the proxy:

```bash
nkl proxy stop
```

## Static Aliases

Register a fixed route to an existing local service:

```bash
nkl alias api 3001
```

Register a path-based alias:

```bash
nkl alias api 3001 --path /v1 --strip-prefix
```

Remove an alias:

```bash
nkl alias api --remove
```

## HTTPS

Enable HTTPS by trusting the local CA, then running the proxy with HTTPS enabled:

```bash
sudo nkl trust
nkl proxy start --https
```

You can also request HTTPS when launching an app:

```bash
nkl run --https npm run dev
```

The current CA trust integration targets macOS and common Linux distributions.

## Hosts Management

`.localhost` works without editing `/etc/hosts`, but hosts management is useful when you add extra local domains.

Sync active routes into `/etc/hosts`:

```bash
sudo nkl hosts sync
```

Remove NKL-managed hosts entries:

```bash
sudo nkl hosts clean
```

## Configuration

Configuration is loaded in this order, from lowest to highest priority:

1. `/etc/nkl/config.toml`
2. `~/.nkl/config.toml`
3. `nkl.toml`
4. `NKL_*` environment variables
5. CLI flags

Example `nkl.toml`:

```toml
[proxy]
port = 1355
https = false
max_hops = 5
domains = ["localhost"]

[app]
port_range_start = 3000
port_range_end = 9999
force = false
ready_timeout = 30

[hosts]
auto_sync = false

[paths]
state_dir = "/absolute/path/to/nkl-state"
```

Useful environment variables:

- `NKL`
- `NKL_PORT`
- `NKL_HTTPS`
- `NKL_DOMAINS`
- `NKL_STATE_DIR`

When NKL launches an app with `nkl run`, it injects:

- `PORT`
- `HOST`
- `NKL_URL`

Set `NKL=skip` or `NKL=0` to run a child command without proxy registration.

## State Directory

By default, NKL stores runtime state in:

- `~/.nkl` for non-privileged proxy ports
- `/tmp/nkl` for privileged proxy ports

Common files include:

- `routes.json`
- `proxy.pid`
- `proxy.port`
- `proxy.log`

## Commands

```text
nkl run <CMD>...
nkl proxy start|stop|logs
nkl alias [NAME] [PORT]
nkl get <NAME>
nkl list
nkl status
nkl trust
nkl config
nkl hosts sync|clean
```

For command-specific help:

```bash
nkl help
nkl run --help
nkl proxy start --help
```

## Inspired By

[vercel-labs/portless](https://github.com/vercel-labs/portless) (TypeScript)

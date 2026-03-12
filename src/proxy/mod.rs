mod daemon;
mod handler;
mod server;
mod websocket;

use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use tokio::sync::RwLock;

use crate::routes::RouteMapping;

pub use daemon::{daemonize_and_start_proxy, ensure_proxy_running, show_logs, stop_proxy};
pub use server::start_proxy;

/// Response header used to identify an NKL proxy.
pub(crate) const NKL_HEADER: &str = "X-NKL";

/// Header tracking how many times a request has passed through NKL.
pub(crate) const NKL_HOPS_HEADER: &str = "x-nkl-hops";

/// Shared in-memory route cache, updated by the background polling task.
pub(crate) type RouteCache = Arc<RwLock<Vec<RouteMapping>>>;

// ---------------------------------------------------------------------------
// Shared response helpers (used by handler and websocket)
// ---------------------------------------------------------------------------

pub(super) fn response(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .header(NKL_HEADER, "1")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

pub(super) fn html_response(status: StatusCode, html: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/html; charset=utf-8")
        .header(NKL_HEADER, "1")
        .body(Full::new(Bytes::from(html.to_string())))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Shared path helpers (used by handler and websocket)
// ---------------------------------------------------------------------------

/// Check if a request path matches a route's path prefix.
///
/// `/api` matches `/api`, `/api/`, `/api/users`, but not `/apifoo`.
pub(super) fn path_matches_prefix(request_path: &str, prefix: &str) -> bool {
    if prefix == "/" {
        return true;
    }
    let norm_prefix = crate::routes::normalize_path_prefix(prefix);
    if request_path == norm_prefix {
        return true;
    }
    request_path.starts_with(&format!("{}/", norm_prefix))
}

/// Strip the matched prefix from the request path.
/// The result is always at least "/".
pub(super) fn strip_path_prefix(request_path: &str, prefix: &str) -> String {
    let norm_prefix = crate::routes::normalize_path_prefix(prefix);
    let stripped = request_path
        .strip_prefix(&norm_prefix)
        .unwrap_or(request_path);
    if stripped.is_empty() || !stripped.starts_with('/') {
        format!("/{}", stripped.trim_start_matches('/'))
    } else {
        stripped.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "handler_tests.rs"]
mod handler_tests;

#[cfg(test)]
#[path = "websocket_tests.rs"]
mod websocket_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_matches_prefix_root() {
        assert!(path_matches_prefix("/", "/"));
        assert!(path_matches_prefix("/anything", "/"));
        assert!(path_matches_prefix("/api/users", "/"));
    }

    #[test]
    fn test_path_matches_prefix_exact() {
        assert!(path_matches_prefix("/api", "/api"));
        assert!(path_matches_prefix("/api/", "/api"));
        assert!(path_matches_prefix("/api/users", "/api"));
    }

    #[test]
    fn test_path_matches_prefix_no_partial() {
        assert!(!path_matches_prefix("/apifoo", "/api"));
        assert!(!path_matches_prefix("/apiversion", "/api"));
    }

    #[test]
    fn test_path_matches_prefix_nested() {
        assert!(path_matches_prefix("/api/v2", "/api/v2"));
        assert!(path_matches_prefix("/api/v2/users", "/api/v2"));
        assert!(!path_matches_prefix("/api/v2users", "/api/v2"));
        assert!(!path_matches_prefix("/api/v1", "/api/v2"));
    }

    #[test]
    fn test_strip_path_prefix_basic() {
        assert_eq!(strip_path_prefix("/api/users", "/api"), "/users");
        assert_eq!(strip_path_prefix("/api", "/api"), "/");
        assert_eq!(strip_path_prefix("/api/", "/api"), "/");
    }

    #[test]
    fn test_strip_path_prefix_nested() {
        assert_eq!(strip_path_prefix("/api/v2/users", "/api/v2"), "/users");
        assert_eq!(strip_path_prefix("/api/v2", "/api/v2"), "/");
    }

    #[test]
    fn test_strip_path_prefix_result_never_empty() {
        assert_eq!(strip_path_prefix("/api", "/api"), "/");
        assert_eq!(strip_path_prefix("/x", "/x"), "/");
    }
}

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};

use crate::pages;
use crate::routes::RouteMapping;

use super::websocket::{handle_upgrade, is_upgrade_request};
use super::{
    NKL_HEADER, NKL_HOPS_HEADER, RouteCache, html_response, path_matches_prefix, response,
    strip_path_prefix,
};

// ---------------------------------------------------------------------------
// Host / hops extraction
// ---------------------------------------------------------------------------

/// Extract the hostname from a request, checking the Host header first
/// (HTTP/1.1), then falling back to the URI authority (HTTP/2 :authority).
/// Returns the hostname without port.
pub(crate) fn extract_host<B>(req: &Request<B>) -> String {
    let raw = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.uri().authority().map(|auth| auth.host().to_string()))
        .unwrap_or_default();

    raw.split(':').next().unwrap_or("").to_string()
}

/// Read the hop count from a request's nkl-hops header.
fn extract_hops<B>(req: &Request<B>) -> u32 {
    req.headers()
        .get(NKL_HOPS_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Route matching
// ---------------------------------------------------------------------------

/// Find the best matching route for a given host and request path.
/// Returns the route with the longest matching path prefix.
fn find_route<'a>(
    host: &str,
    req_path: &str,
    routes: &'a [RouteMapping],
) -> Option<&'a RouteMapping> {
    let mut candidates: Vec<_> = routes
        .iter()
        .filter(|r| r.hostname == host || host.ends_with(&format!(".{}", r.hostname)))
        .filter(|r| path_matches_prefix(req_path, &r.path_prefix))
        .collect();
    candidates.sort_by(|a, b| b.path_prefix.len().cmp(&a.path_prefix.len()));
    candidates.into_iter().next()
}

// ---------------------------------------------------------------------------
// Request forwarding helpers
// ---------------------------------------------------------------------------

/// Build the forwarded path and query string, optionally stripping the
/// matched route prefix.
fn build_forwarded_path_and_query(
    req_path: &str,
    query: Option<&str>,
    route: &RouteMapping,
) -> String {
    let forwarded_path = if route.strip_prefix && route.path_prefix != "/" {
        strip_path_prefix(req_path, &route.path_prefix)
    } else {
        req_path.to_string()
    };
    match query {
        Some(q) => format!("{}?{}", forwarded_path, q),
        None => forwarded_path,
    }
}

/// Build the forwarded HTTP request with rewritten headers.
fn build_forwarded_request(
    parts: &hyper::http::request::Parts,
    target_uri: &str,
    host: &str,
    proxy_port: u16,
    hops: u32,
    route: &RouteMapping,
    body_bytes: Bytes,
    is_tls: bool,
) -> Request<Full<Bytes>> {
    let mut proxy_req = Request::builder()
        .method(parts.method.clone())
        .uri(target_uri);

    // Copy original headers, skipping pseudo-headers and optionally Host.
    for (key, value) in &parts.headers {
        if key.as_str().starts_with(':') {
            continue;
        }
        if route.change_origin && key == hyper::header::HOST {
            continue;
        }
        proxy_req = proxy_req.header(key, value);
    }

    if route.change_origin {
        proxy_req = proxy_req.header(hyper::header::HOST, format!("127.0.0.1:{}", route.port));
    }

    // X-Forwarded-* headers
    let fwd_for = parts
        .headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|existing| format!("{}, 127.0.0.1", existing))
        .unwrap_or_else(|| "127.0.0.1".to_string());
    proxy_req = proxy_req.header("x-forwarded-for", fwd_for);

    if !parts.headers.contains_key("x-forwarded-proto") {
        proxy_req = proxy_req.header("x-forwarded-proto", if is_tls { "https" } else { "http" });
    }
    if !parts.headers.contains_key("x-forwarded-host") {
        proxy_req = proxy_req.header("x-forwarded-host", host);
    }
    if !parts.headers.contains_key("x-forwarded-port") {
        proxy_req = proxy_req.header("x-forwarded-port", proxy_port.to_string());
    }

    proxy_req = proxy_req
        .header(NKL_HOPS_HEADER, (hops + 1).to_string())
        .header(NKL_HEADER, "1");

    proxy_req.body(Full::new(body_bytes)).unwrap()
}

// ---------------------------------------------------------------------------
// Main request handler
// ---------------------------------------------------------------------------

pub(super) async fn handle_request(
    req: Request<Incoming>,
    proxy_port: u16,
    max_hops: u32,
    route_cache: RouteCache,
    is_tls: bool,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let host = extract_host(&req);
    if host.is_empty() {
        return Ok(response(StatusCode::BAD_REQUEST, "Missing Host header"));
    }

    let hops = extract_hops(&req);
    if hops >= max_hops {
        let body_html = pages::render_loop_detected_body();
        let html = pages::render_page(508, "Loop Detected", &body_html);
        return Ok(html_response(
            StatusCode::from_u16(508).unwrap_or(StatusCode::BAD_GATEWAY),
            &html,
        ));
    }

    let routes = route_cache.read().await.clone();
    let req_path = req.uri().path().to_string();

    let route = find_route(&host, &req_path, &routes);
    let Some(route) = route else {
        let body_html = pages::render_not_found_body(&host, &routes);
        let html = pages::render_page(404, "Not Found", &body_html);
        return Ok(html_response(StatusCode::NOT_FOUND, &html));
    };

    if is_upgrade_request(&req) {
        tracing::debug!("WebSocket upgrade request for {}", host);
        return handle_upgrade(req, proxy_port, host, hops, route, is_tls).await;
    }

    let path_and_query = build_forwarded_path_and_query(&req_path, req.uri().query(), route);
    let target_uri = format!("http://127.0.0.1:{}{}", route.port, path_and_query);

    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();

    let (parts, body) = req.into_parts();
    let body_bytes = body
        .collect()
        .await
        .map(|c| c.to_bytes())
        .unwrap_or_default();

    let proxy_req = build_forwarded_request(
        &parts,
        &target_uri,
        &host,
        proxy_port,
        hops,
        route,
        body_bytes,
        is_tls,
    );

    match client.request(proxy_req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let body_bytes = body
                .collect()
                .await
                .map(|c| c.to_bytes())
                .unwrap_or_default();
            let mut response = Response::builder().status(parts.status);
            for (key, value) in &parts.headers {
                response = response.header(key, value);
            }
            response = response.header(NKL_HEADER, "1");
            Ok(response.body(Full::new(body_bytes)).unwrap())
        }
        Err(_) => {
            let body_html = pages::render_bad_gateway_body(&host, route.port);
            let html = pages::render_page(502, "Bad Gateway", &body_html);
            Ok(html_response(StatusCode::BAD_GATEWAY, &html))
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
    fn test_handle_request_authority_fallback() {
        let uri: hyper::Uri = "http://myapp.localhost:8080/hello".parse().unwrap();
        let req = Request::builder().method("GET").uri(uri).body(()).unwrap();
        assert_eq!(extract_host(&req), "myapp.localhost");
    }

    #[test]
    fn test_host_extraction_header_takes_precedence() {
        let uri: hyper::Uri = "http://other.localhost:8080/path".parse().unwrap();
        let req = Request::builder()
            .method("GET")
            .uri(uri)
            .header(hyper::header::HOST, "myapp.localhost:1234")
            .body(())
            .unwrap();
        assert_eq!(extract_host(&req), "myapp.localhost");
    }

    #[test]
    fn test_host_extraction_empty_when_neither_present() {
        let req = Request::builder()
            .method("GET")
            .uri("/path")
            .body(())
            .unwrap();
        assert!(extract_host(&req).is_empty());
    }

    #[test]
    fn test_pseudo_headers_stripped_in_forwarding() {
        let mut headers = hyper::HeaderMap::new();
        headers.insert("x-custom", "value".parse().unwrap());
        headers.insert(hyper::header::HOST, "myapp.localhost".parse().unwrap());

        let mut forwarded_headers: Vec<String> = Vec::new();
        for (key, _value) in &headers {
            if key.as_str().starts_with(':') {
                continue;
            }
            forwarded_headers.push(key.to_string());
        }

        assert!(forwarded_headers.contains(&"x-custom".to_string()));
        assert!(forwarded_headers.contains(&"host".to_string()));
    }
}

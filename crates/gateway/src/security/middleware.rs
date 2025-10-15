// src/security/middleware.rs
use axum::{
    extract::Request, http::HeaderValue, middleware::Next, response::Response
};
use super::config::SecurityHeadersConfig;

pub async fn security_headers_middleware(
    request: Request,
    next: Next,
) -> Response {
    let config = super::config::production_security_config();
    configurable_security_headers(request, next, &config).await
}

pub async fn configurable_security_headers(
    request: Request,
    next: Next,
    config: &SecurityHeadersConfig,
) -> Response {
    let mut response = next.run(request).await;
    add_security_headers(response.headers_mut(), config);
    response
}

fn add_security_headers(headers: &mut axum::http::HeaderMap, config: &SecurityHeadersConfig) {
    // 1. Content Security Policy
    if config.enable_csp &&  let Ok(header_value) = HeaderValue::from_str(&config.csp_directives) {
        headers.insert("content-security-policy", header_value);
    }

    // 2. Strict Transport Security (仅在 HTTPS 时启用)
    if config.enable_hsts {
        let hsts_value = format!(
            "max-age={}; includeSubDomains{}",
            config.hsts_max_age.as_secs(),
            if config.hsts_max_age.as_secs() >= 31536000 { "; preload" } else { "" }
        );
        if let Ok(header_value) = HeaderValue::from_str(&hsts_value) {
            headers.insert("strict-transport-security", header_value);
        }
    }

    // 3. X-Content-Type-Options
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));

    // 4. X-Frame-Options
    let frame_options_value = match &config.frame_options {
        super::config::FrameOptions::Deny => "DENY",
        super::config::FrameOptions::SameOrigin => "SAMEORIGIN",
    };
    headers.insert("x-frame-options", HeaderValue::from_static(frame_options_value));

    // 5. X-XSS-Protection
    if config.enable_xss_protection {
        headers.insert("x-xss-protection", HeaderValue::from_static("1; mode=block"));
    }

    // 6. Referrer-Policy
    headers.insert("referrer-policy", HeaderValue::from_static("strict-origin-when-cross-origin"));

    // 7. Permissions-Policy
    if config.enable_permissions_policy && let Ok(header_value) = HeaderValue::from_str(&config.permissions_policy) {
        headers.insert("permissions-policy", header_value);
    }

    // 8. remove sensitive headers
    remove_sensitive_headers(headers);
}

fn remove_sensitive_headers(headers: &mut axum::http::HeaderMap) {
    headers.remove("server");
    headers.remove("x-powered-by");
    headers.remove("x-aspnet-version");
    headers.remove("x-aspnetmvc-version");
}
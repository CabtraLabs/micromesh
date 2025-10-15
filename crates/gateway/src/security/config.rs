// src/security/config.rs
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SecurityHeadersConfig {
    pub enable_csp: bool,
    pub enable_hsts: bool,
    pub enable_xss_protection: bool,
    pub frame_options: FrameOptions,
    pub csp_directives: String,
    pub hsts_max_age: Duration,
    pub enable_permissions_policy: bool,
    pub permissions_policy: String,
}

#[derive(Debug, Clone)]
pub enum FrameOptions {
    Deny,
    SameOrigin,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enable_csp: true,
            enable_hsts: true,
            enable_xss_protection: true,
            frame_options: FrameOptions::SameOrigin,
            csp_directives: default_csp_directives(),
            hsts_max_age: Duration::from_secs(31536000), // 1 year
            enable_permissions_policy: true,
            permissions_policy: default_permissions_policy(),
        }
    }
}

/// Recommended security configuration for production environment
pub fn production_security_config() -> SecurityHeadersConfig {
    SecurityHeadersConfig {
        enable_csp: true,
        enable_hsts: true, // Note: Enable HSTS only when using HTTPS
        enable_xss_protection: true,
        frame_options: FrameOptions::Deny,
        csp_directives: production_csp_directives(),
        hsts_max_age: Duration::from_secs(63072000), // 2 years
        enable_permissions_policy: true,
        permissions_policy: production_permissions_policy(),
    }
}

fn default_csp_directives() -> String {
    [
        "default-src 'self'",
        "script-src 'self' 'unsafe-inline'",
        "style-src 'self' 'unsafe-inline'",
        "img-src 'self' data: https:",
        "font-src 'self'",
        "connect-src 'self'",
        "object-src 'none'",
        "base-uri 'self'",
        "form-action 'self'",
        "frame-ancestors 'none'",
    ].join("; ")
}

fn production_csp_directives() -> String {
    let origins = utils::vars::get_allow_origins();
    let connec_src = if origins.contains(&"*".to_string()) {
        "connect-src * data: blob:".to_string().to_string()
    } else {
        format!("connect-src 'self' {origins}")
    };
    [
        "default-src 'self'",
        "script-src 'self'",
        "style-src 'self'",
        "img-src 'self' data: https:",
        "font-src 'self'",
        connec_src.as_str(),
        "object-src 'none'",
        "child-src 'none'",
        "frame-src 'none'",
        "frame-ancestors 'none'",
        "base-uri 'self'",
        "form-action 'self'",
        "upgrade-insecure-requests", // Automatically upgrade HTTP to HTTPS
    ].join("; ")
}

fn default_permissions_policy() -> String {
    [
        "geolocation=()",
        "microphone=()", 
        "camera=()",
        "payment=()",
        "usb=()",
        "serial=()",
    ].join(", ")
}

fn production_permissions_policy() -> String {
    [
        "geolocation=()",
        "microphone=()",
        "camera=()", 
        "payment=()",
        "usb=()",
        "serial=()",
        "magnetometer=()",
        "gyroscope=()",
        "accelerometer=()",
    ].join(", ")
}
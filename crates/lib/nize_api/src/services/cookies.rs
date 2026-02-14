// @zen-component: CFG-CookieAuth
//
//! Cookie service — set/get/clear httpOnly auth cookies.
//!
//! Cookie names match the ref project convention: `nize_access`, `nize_refresh`.

use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;

/// Cookie name for the access token.
pub const ACCESS_COOKIE: &str = "nize_access";
/// Cookie name for the refresh token.
pub const REFRESH_COOKIE: &str = "nize_refresh";

/// Build a httpOnly cookie for the access token.
pub fn access_cookie(token: &str, max_age_secs: i64) -> Cookie<'static> {
    Cookie::build((ACCESS_COOKIE.to_string(), token.to_string()))
        .http_only(true)
        .secure(false) // TODO: set true in production
        .same_site(SameSite::Lax)
        .path("/".to_string())
        .max_age(Duration::seconds(max_age_secs))
        .build()
}

/// Build a httpOnly cookie for the refresh token (30 days).
pub fn refresh_cookie(token: &str) -> Cookie<'static> {
    Cookie::build((REFRESH_COOKIE.to_string(), token.to_string()))
        .http_only(true)
        .secure(false) // TODO: set true in production
        .same_site(SameSite::Lax)
        .path("/".to_string())
        .max_age(Duration::days(30))
        .build()
}

/// Build expired cookies to clear auth state.
pub fn clear_access_cookie() -> Cookie<'static> {
    Cookie::build((ACCESS_COOKIE.to_string(), String::new()))
        .http_only(true)
        .secure(false)
        .same_site(SameSite::Lax)
        .path("/".to_string())
        .max_age(Duration::ZERO)
        .build()
}

/// Build expired cookie to clear refresh token.
pub fn clear_refresh_cookie() -> Cookie<'static> {
    Cookie::build((REFRESH_COOKIE.to_string(), String::new()))
        .http_only(true)
        .secure(false)
        .same_site(SameSite::Lax)
        .path("/".to_string())
        .max_age(Duration::ZERO)
        .build()
}

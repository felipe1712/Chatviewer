use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use chrono::{Duration, Utc};
use std::sync::Arc;

use crate::crypto::{LicensePayload, LicenseSigner, LicenseTier};
use crate::handlers::checkout::AppState;

const LANDING_HTML: &str = include_str!("../../templates/landing.html");
const PORTAL_HTML: &str = include_str!("../../templates/portal.html");
const ADMIN_HTML: &str = include_str!("../../templates/admin.html");

const ROBOTS_TXT: &str = "User-agent: *\nAllow: /\nDisallow: /admin\nDisallow: /api/\n\nSitemap: http://chatproof.pro/sitemap.xml\n";

const SITEMAP_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n  <url>\n    <loc>http://chatproof.pro/</loc>\n    <lastmod>2026-11-09</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>1.0</priority>\n  </url>\n  <url>\n    <loc>http://chatproof.pro/portal</loc>\n    <lastmod>2026-11-09</lastmod>\n    <changefreq>monthly</changefreq>\n    <priority>0.8</priority>\n  </url>\n</urlset>\n";

pub async fn serve_landing() -> Html<&'static str> {
    Html(LANDING_HTML)
}

pub async fn serve_portal() -> Html<&'static str> {
    Html(PORTAL_HTML)
}

pub async fn serve_admin() -> Html<&'static str> {
    Html(ADMIN_HTML)
}

pub async fn serve_robots() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        ROBOTS_TXT,
    )
        .into_response()
}

pub async fn serve_sitemap() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        SITEMAP_XML,
    )
        .into_response()
}

/// Descarga de la versión de evaluación gratuita (Free 7 días)
pub async fn download_free_evaluation(
    State(state): State<Arc<AppState>>,
) -> Response {
    let now = Utc::now();
    let payload = LicensePayload {
        license_id: format!("FREE-EVAL-{}", &uuid::Uuid::new_v4().to_string()[..8].to_uppercase()),
        customer_name: "Evaluación Pericial (7 Días)".to_string(),
        customer_email: "evaluacion@chatproof.pro".to_string(),
        tier: LicenseTier::Free,
        seats: 1,
        machine_fingerprint: None,
        allowed_machines: Vec::new(),
        issued_at: now,
        expires_at: Some(now + Duration::days(7)),
        enabled_features: LicenseTier::Free.default_features(),
    };

    let signed = LicenseSigner::sign(payload, state.config.master_license_secret.as_bytes());
    let lic_json = serde_json::to_string_pretty(&signed).unwrap_or_default();

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"chatproof_free_trial.lic\"",
            ),
        ],
        lic_json,
    )
        .into_response()
}

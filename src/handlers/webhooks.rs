use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::handlers::checkout::AppState;

type HmacSha256 = Hmac<Sha256>;

pub async fn handle_stripe_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let sig_header = headers
        .get("stripe-signature")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    if !state.config.stripe_webhook_secret.starts_with("whsec_test_chatviewer_placeholder") {
        let is_valid = verify_stripe_signature(&body, sig_header, &state.config.stripe_webhook_secret);
        if !is_valid {
            return (StatusCode::BAD_REQUEST, "Firma de webhook inválida").into_response();
        }
    }

    let payload: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "JSON mal formado").into_response(),
    };

    let event_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
    let now = Utc::now();

    let _ = sqlx::query(
        "INSERT INTO webhook_logs (id, provider, event_type, payload, status, created_at) VALUES (?1, 'stripe', ?2, ?3, 'processed', ?4)"
    )
    .bind(Uuid::new_v4().to_string())
    .bind(event_type)
    .bind(&body)
    .bind(now)
    .execute(&state.db)
    .await;

    match event_type {
        "checkout.session.completed" => {
            if let Some(session_obj) = payload.get("data").and_then(|d| d.get("object")) {
                let session_id = session_obj.get("id").and_then(|s| s.as_str()).unwrap_or("");
                let _ = sqlx::query(
                    "UPDATE orders SET status = 'paid', updated_at = ?1 WHERE provider_session_id = ?2"
                )
                .bind(now)
                .bind(session_id)
                .execute(&state.db)
                .await;
            }
        }
        "invoice.payment_succeeded" => {
            if let Some(inv_obj) = payload.get("data").and_then(|d| d.get("object")) {
                if let Some(email) = inv_obj.get("customer_email").and_then(|e| e.as_str()) {
                    let new_exp = Utc::now() + Duration::days(365);
                    let _ = sqlx::query(
                        r#"
                        UPDATE licenses SET expires_at = ?1
                        WHERE customer_id IN (SELECT id FROM customers WHERE email = ?2)
                        "#
                    )
                    .bind(new_exp)
                    .bind(email)
                    .execute(&state.db)
                    .await;
                }
            }
        }
        "customer.subscription.deleted" | "invoice.payment_failed" => {
            if let Some(sub_obj) = payload.get("data").and_then(|d| d.get("object")) {
                if let Some(email) = sub_obj.get("customer_email").and_then(|e| e.as_str()) {
                    let _ = sqlx::query(
                        r#"
                        UPDATE licenses SET is_revoked = 1, revocation_reason = 'Suscripción cancelada o impagada'
                        WHERE customer_id IN (SELECT id FROM customers WHERE email = ?1)
                        "#
                    )
                    .bind(email)
                    .execute(&state.db)
                    .await;
                }
            }
        }
        _ => {}
    }

    (StatusCode::OK, Json(serde_json::json!({ "received": true }))).into_response()
}

fn verify_stripe_signature(payload: &str, sig_header: &str, secret: &str) -> bool {
    let mut timestamp = "";
    let mut sig_hex = "";

    for item in sig_header.split(',') {
        let mut parts = item.split('=');
        match (parts.next(), parts.next()) {
            (Some("t"), Some(t)) => timestamp = t,
            (Some("v1"), Some(v)) => sig_hex = v,
            _ => {}
        }
    }

    if timestamp.is_empty() || sig_hex.is_empty() {
        return false;
    }

    let signed_payload = format!("{}.{}", timestamp, payload);
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    expected.as_bytes().ct_eq(sig_hex.as_bytes()).unwrap_u8() == 1
}

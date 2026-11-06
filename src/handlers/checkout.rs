use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use uuid::Uuid;

use crate::config::Config;
use crate::crypto::{LicensePayload, LicenseSigner, LicenseTier};
use crate::email::EmailService;
use crate::models::{CheckoutResponse, CreateCheckoutRequest};

pub struct AppState {
    pub db: SqlitePool,
    pub config: Arc<Config>,
}

pub async fn create_checkout(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCheckoutRequest>,
) -> Response {
    let order_id = format!("ORD-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());
    let customer_id = format!("CUST-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());

    let is_monthly = req.billing_cycle.as_deref().map(|c| c == "monthly").unwrap_or(false);
    let (default_amount_cents, tier_enum, default_seats) = match (req.tier.to_lowercase().as_str(), is_monthly) {
        ("laboratorio", true) => (59900, LicenseTier::Laboratorio, 5),
        ("laboratorio", false) => (189900, LicenseTier::Laboratorio, 5),
        (_, true) => (24900, LicenseTier::Peritaje, 1),
        (_, false) => (49900, LicenseTier::Peritaje, 1),
    };
    let amount_cents = req.amount_override.map(|a| a * 100).unwrap_or(default_amount_cents);
    let seats = req.seats.unwrap_or(default_seats) as i64;
    let now = Utc::now();

    // 1. Guardar o actualizar cliente
    let _ = sqlx::query(
        r#"
        INSERT INTO customers (id, email, name, company, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(email) DO UPDATE SET name = excluded.name, company = excluded.company
        "#,
    )
    .bind(&customer_id)
    .bind(&req.email)
    .bind(&req.name)
    .bind(&req.company)
    .bind(now)
    .execute(&state.db)
    .await;

    // Obtener ID efectivo del cliente (por si ya existía)
    let actual_cust_id = match sqlx::query("SELECT id FROM customers WHERE email = ?1")
        .bind(&req.email)
        .fetch_one(&state.db)
        .await
    {
        Ok(row) => row.get::<String, _>("id"),
        Err(_) => customer_id,
    };

    let session_id = format!("sess_{}", Uuid::new_v4().to_string().replace('-', ""));
    let provider = "stripe".to_string();

    // 2. Determinar URL de Checkout de Stripe
    let checkout_url = if state.config.stripe_secret_key.starts_with("sk_test_chatviewer_placeholder") {
        format!("{}/portal?session_id={}&email={}&simulated=true", state.config.public_url, session_id, req.email)
    } else {
        let client = reqwest::Client::new();
        let params = [
            ("payment_method_types[0]", "card"),
            ("line_items[0][price_data][currency]", "usd"),
            ("line_items[0][price_data][product_data][name]", &format!("ChatViewer Pro - Nivel {}", tier_enum.as_str().to_uppercase())),
            ("line_items[0][price_data][unit_amount]", &amount_cents.to_string()),
            ("line_items[0][quantity]", "1"),
            ("mode", "payment"),
            ("customer_email", &req.email),
            ("success_url", &state.config.stripe_success_url),
            ("cancel_url", &state.config.stripe_cancel_url),
            ("metadata[order_id]", &order_id),
            ("metadata[tier]", tier_enum.as_str()),
            ("metadata[seats]", &seats.to_string()),
        ];

        match client
            .post("https://api.stripe.com/v1/checkout/sessions")
            .basic_auth(&state.config.stripe_secret_key, Some(""))
            .form(&params)
            .send()
            .await
        {
            Ok(resp) => {
                if let Ok(json_body) = resp.json::<serde_json::Value>().await {
                    json_body.get("url").and_then(|u| u.as_str()).map(|s| s.to_string())
                        .unwrap_or_else(|| format!("{}/portal?session_id={}", state.config.public_url, session_id))
                } else {
                    format!("{}/portal?session_id={}", state.config.public_url, session_id)
                }
            }
            Err(_) => format!("{}/portal?session_id={}", state.config.public_url, session_id),
        }
    };

    // 3. Crear Orden en DB
    let _ = sqlx::query(
        r#"
        INSERT INTO orders (id, customer_id, provider, provider_session_id, tier, seats, amount_cents, currency, status, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'paid', ?9, ?9)
        "#,
    )
    .bind(&order_id)
    .bind(&actual_cust_id)
    .bind(&provider)
    .bind(&session_id)
    .bind(tier_enum.as_str())
    .bind(seats)
    .bind(amount_cents)
    .bind("USD")
    .bind(now)
    .execute(&state.db)
    .await;

    // 4. Crear Licencia Inicial (vigencia mensual 30 días o anual 365 días)
    let license_id = format!("LIC-{}-{}", tier_enum.as_str().to_uppercase(), &Uuid::new_v4().to_string()[..8].to_uppercase());
    let valid_days = if is_monthly { 30 } else { 365 };
    let expires_at = now + Duration::days(valid_days);
    let lic_payload = LicensePayload {
        license_id: license_id.clone(),
        customer_name: req.name.clone(),
        customer_email: req.email.clone(),
        tier: tier_enum,
        seats: seats as u32,
        machine_fingerprint: None,
        allowed_machines: Vec::new(),
        issued_at: now,
        expires_at: Some(expires_at),
        enabled_features: match req.tier.to_lowercase().as_str() {
            "laboratorio" => LicenseTier::Laboratorio.default_features(),
            _ => LicenseTier::Peritaje.default_features(),
        },
    };
    let signed = LicenseSigner::sign(lic_payload, state.config.master_license_secret.as_bytes());
    let lic_json = serde_json::to_string_pretty(&signed).unwrap_or_default();

    let _ = sqlx::query(
        r#"
        INSERT INTO licenses (id, order_id, customer_id, tier, seats, issued_at, expires_at, signature, is_revoked, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)
        "#,
    )
    .bind(&license_id)
    .bind(&order_id)
    .bind(&actual_cust_id)
    .bind(req.tier.to_lowercase())
    .bind(seats)
    .bind(now)
    .bind(expires_at)
    .bind(&signed.signature)
    .bind(now)
    .execute(&state.db)
    .await;

    // 5. Envío transaccional del archivo de licencia por correo mediante Brevo
    let _ = EmailService::send_license_email(
        &state.config,
        &req.email,
        &req.name,
        &license_id,
        &req.tier,
        &lic_json,
    )
    .await;

    (
        StatusCode::OK,
        Json(CheckoutResponse {
            checkout_url,
            order_id,
            session_id,
            provider,
        }),
    )
        .into_response()
}

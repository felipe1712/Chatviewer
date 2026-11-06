use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::crypto::{LicensePayload, LicenseSigner, LicenseTier};
use crate::handlers::checkout::AppState;
use crate::models::{AssignSeatRequest, LicenseRecord, LicenseSeat};

#[derive(Debug, Deserialize)]
pub struct PortalQuery {
    pub email: Option<String>,
    pub session_id: Option<String>,
}

pub async fn list_portal_licenses(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PortalQuery>,
) -> Response {
    let email = query.email.unwrap_or_default();
    let session_id = query.session_id.unwrap_or_default();

    let licenses: Vec<LicenseRecord> = match sqlx::query_as::<_, LicenseRecord>(
        r#"
        SELECT l.id, l.order_id, l.customer_id, l.tier, l.seats, l.issued_at, l.expires_at, l.signature, l.is_revoked, l.revocation_reason, l.created_at
        FROM licenses l
        LEFT JOIN customers c ON l.customer_id = c.id
        LEFT JOIN orders o ON l.order_id = o.id
        WHERE c.email = ?1 OR o.provider_session_id = ?2
        ORDER BY l.created_at DESC
        "#,
    )
    .bind(&email)
    .bind(&session_id)
    .fetch_all(&state.db)
    .await {
        Ok(l) => l,
        Err(_) => Vec::new(),
    };

    let mut result = Vec::new();
    let now = Utc::now();

    for lic in licenses {
        let seats: Vec<LicenseSeat> = sqlx::query_as::<_, LicenseSeat>(
            r#"
            SELECT id, license_id, machine_fingerprint, dongle_id, label, assigned_at
            FROM license_seats
            WHERE license_id = ?1
            "#,
        )
        .bind(&lic.id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let days_remaining = lic.expires_at.map(|exp| (exp - now).num_days());

        result.push(json!({
            "id": lic.id,
            "tier": lic.tier,
            "seats": lic.seats,
            "issued_at": lic.issued_at,
            "expires_at": lic.expires_at,
            "days_remaining": days_remaining,
            "is_revoked": lic.is_revoked,
            "seats_assigned": seats,
        }));
    }

    (StatusCode::OK, Json(json!({ "licenses": result }))).into_response()
}

pub async fn assign_seat(
    State(state): State<Arc<AppState>>,
    Path(license_id): Path<String>,
    Json(payload): Json<AssignSeatRequest>,
) -> Response {
    let lic = match sqlx::query_as::<_, LicenseRecord>(
        "SELECT id, order_id, customer_id, tier, seats, issued_at, expires_at, signature, is_revoked, revocation_reason, created_at FROM licenses WHERE id = ?1",
    )
    .bind(&license_id)
    .fetch_optional(&state.db)
    .await {
        Ok(Some(l)) => l,
        _ => return (StatusCode::NOT_FOUND, Json(json!({ "message": "Licencia no encontrada" }))).into_response(),
    };

    if lic.is_revoked {
        return (StatusCode::FORBIDDEN, Json(json!({ "message": "Esta licencia ha sido revocada" }))).into_response();
    }

    // Verificar cupo de puestos
    let current_seats_count: i64 = match sqlx::query("SELECT COUNT(*) as count FROM license_seats WHERE license_id = ?1")
        .bind(&license_id)
        .fetch_one(&state.db)
        .await
    {
        Ok(row) => row.get::<i64, _>("count"),
        Err(_) => 0,
    };

    if current_seats_count >= lic.seats {
        return (StatusCode::BAD_REQUEST, Json(json!({ "message": "Se ha alcanzado el límite máximo de puestos autorizados" }))).into_response();
    }

    let seat_id = format!("SEAT-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());
    let now = Utc::now();

    let insert_res = sqlx::query(
        r#"
        INSERT INTO license_seats (id, license_id, machine_fingerprint, dongle_id, label, assigned_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(license_id, machine_fingerprint) DO UPDATE SET label = excluded.label, dongle_id = excluded.dongle_id
        "#,
    )
    .bind(&seat_id)
    .bind(&license_id)
    .bind(&payload.machine_fingerprint)
    .bind(&payload.dongle_id)
    .bind(&payload.label)
    .bind(now)
    .execute(&state.db)
    .await;

    if insert_res.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "message": "Error al registrar la huella de estación" }))).into_response();
    }

    // Recalcular firma con la lista de huellas actualizada
    let assigned_rows = sqlx::query("SELECT machine_fingerprint FROM license_seats WHERE license_id = ?1")
        .bind(&license_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let assigned_fps: Vec<String> = assigned_rows.iter().map(|r| r.get::<String, _>("machine_fingerprint")).collect();

    let tier_enum = match lic.tier.to_lowercase().as_str() {
        "laboratorio" => LicenseTier::Laboratorio,
        _ => LicenseTier::Peritaje,
    };

    let cust_name = match sqlx::query("SELECT name FROM customers WHERE id = ?1")
        .bind(&lic.customer_id)
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<String, _>("name"),
        Err(_) => "Titular Pericial".to_string(),
    };

    let cust_email = match sqlx::query("SELECT email FROM customers WHERE id = ?1")
        .bind(&lic.customer_id)
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<String, _>("email"),
        Err(_) => "perito@laboratorio.com".to_string(),
    };

    let lic_payload = LicensePayload {
        license_id: lic.id.clone(),
        customer_name: cust_name,
        customer_email: cust_email,
        tier: tier_enum,
        seats: lic.seats as u32,
        machine_fingerprint: assigned_fps.first().cloned(),
        allowed_machines: assigned_fps,
        issued_at: lic.issued_at,
        expires_at: lic.expires_at,
        enabled_features: match lic.tier.to_lowercase().as_str() {
            "laboratorio" => LicenseTier::Laboratorio.default_features(),
            _ => LicenseTier::Peritaje.default_features(),
        },
    };

    let signed = LicenseSigner::sign(lic_payload, state.config.master_license_secret.as_bytes());

    let _ = sqlx::query("UPDATE licenses SET signature = ?1 WHERE id = ?2")
        .bind(&signed.signature)
        .bind(&lic.id)
        .execute(&state.db)
        .await;

    (StatusCode::OK, Json(json!({ "message": "Puesto vinculado y licencia firmada con éxito", "license_id": lic.id }))).into_response()
}

pub async fn download_license(
    State(state): State<Arc<AppState>>,
    Path(license_id): Path<String>,
) -> Response {
    let lic = match sqlx::query_as::<_, LicenseRecord>(
        "SELECT id, order_id, customer_id, tier, seats, issued_at, expires_at, signature, is_revoked, revocation_reason, created_at FROM licenses WHERE id = ?1",
    )
    .bind(&license_id)
    .fetch_optional(&state.db)
    .await {
        Ok(Some(l)) => l,
        _ => return (StatusCode::NOT_FOUND, "Licencia no encontrada").into_response(),
    };

    let assigned_rows = sqlx::query("SELECT machine_fingerprint FROM license_seats WHERE license_id = ?1")
        .bind(&license_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let assigned_fps: Vec<String> = assigned_rows.iter().map(|r| r.get::<String, _>("machine_fingerprint")).collect();

    let cust_name = match sqlx::query("SELECT name FROM customers WHERE id = ?1")
        .bind(&lic.customer_id)
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<String, _>("name"),
        Err(_) => "Titular Pericial".to_string(),
    };

    let cust_email = match sqlx::query("SELECT email FROM customers WHERE id = ?1")
        .bind(&lic.customer_id)
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<String, _>("email"),
        Err(_) => "perito@laboratorio.com".to_string(),
    };

    let tier_enum = match lic.tier.to_lowercase().as_str() {
        "laboratorio" => LicenseTier::Laboratorio,
        _ => LicenseTier::Peritaje,
    };

    let lic_payload = LicensePayload {
        license_id: lic.id.clone(),
        customer_name: cust_name,
        customer_email: cust_email,
        tier: tier_enum,
        seats: lic.seats as u32,
        machine_fingerprint: assigned_fps.first().cloned(),
        allowed_machines: assigned_fps,
        issued_at: lic.issued_at,
        expires_at: lic.expires_at,
        enabled_features: match lic.tier.to_lowercase().as_str() {
            "laboratorio" => LicenseTier::Laboratorio.default_features(),
            _ => LicenseTier::Peritaje.default_features(),
        },
    };

    let signed = LicenseSigner::sign(lic_payload, state.config.master_license_secret.as_bytes());
    let lic_json = serde_json::to_string_pretty(&signed).unwrap_or_default();

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"chatviewer_license_{}.lic\"", lic.id),
            ),
        ],
        lic_json,
    )
        .into_response()
}

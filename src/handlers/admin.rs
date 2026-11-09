use axum::{
    extract::{Json, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::Row;
use std::io::{Cursor, Write};
use std::sync::Arc;
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::crypto::{LicensePayload, LicenseSigner, LicenseTier};
use crate::handlers::checkout::AppState;
use crate::models::{BatchGenerateRequest, CrlRevocation, RevokeLicenseRequest};

async fn check_auth(headers: &HeaderMap, state: &AppState) -> bool {
    if let Some(auth) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return crate::auth::validate_token(&state.db, &state.config.admin_token, token).await.is_some();
        }
    }
    false
}

pub async fn admin_overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if !check_auth(&headers, &state).await {
        return (StatusCode::UNAUTHORIZED, "Acceso no autorizado").into_response();
    }

    let total_licenses: i64 = match sqlx::query("SELECT COUNT(*) as c FROM licenses")
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<i64, _>("c"),
        Err(_) => 0,
    };

    let total_seats: i64 = match sqlx::query("SELECT COUNT(*) as c FROM license_seats")
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<i64, _>("c"),
        Err(_) => 0,
    };

    let revenue_cents: i64 = match sqlx::query("SELECT COALESCE(SUM(amount_cents), 0) as s FROM orders WHERE status = 'paid'")
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<i64, _>("s"),
        Err(_) => 0,
    };

    let total_revocations: i64 = match sqlx::query("SELECT COUNT(*) as c FROM crl_revocations")
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<i64, _>("c"),
        Err(_) => 0,
    };

    let license_rows = sqlx::query(
        r#"
        SELECT l.id, l.tier, l.seats, l.issued_at, l.expires_at, l.is_revoked,
               c.name as customer_name, c.email as customer_email,
               (SELECT COUNT(*) FROM license_seats ls WHERE ls.license_id = l.id) as seats_assigned
        FROM licenses l
        LEFT JOIN customers c ON l.customer_id = c.id
        ORDER BY l.created_at DESC
        LIMIT 50
        "#
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let list_json = license_rows.iter().map(|l| json!({
        "id": l.get::<String, _>("id"),
        "tier": l.get::<String, _>("tier"),
        "seats": l.get::<i64, _>("seats"),
        "issued_at": l.get::<chrono::DateTime<chrono::Utc>, _>("issued_at"),
        "expires_at": l.get::<Option<chrono::DateTime<chrono::Utc>>, _>("expires_at"),
        "is_revoked": l.get::<bool, _>("is_revoked"),
        "customer_name": l.get::<Option<String>, _>("customer_name").unwrap_or_else(|| "Titular".to_string()),
        "customer_email": l.get::<Option<String>, _>("customer_email").unwrap_or_else(|| "email@peritaje.com".to_string()),
        "seats_assigned": l.get::<i64, _>("seats_assigned"),
    })).collect::<Vec<_>>();

    (StatusCode::OK, Json(json!({
        "total_licenses": total_licenses,
        "total_seats": total_seats,
        "revenue_cents": revenue_cents,
        "total_revocations": total_revocations,
        "licenses": list_json
    }))).into_response()
}

pub async fn batch_generate(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<BatchGenerateRequest>,
) -> Response {
    if !check_auth(&headers, &state).await {
        return (StatusCode::UNAUTHORIZED, "Acceso no autorizado").into_response();
    }

    let customer_id = format!("CUST-BATCH-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());
    let now = Utc::now();
    let days = req.valid_days.unwrap_or(365);
    let expires_at = now + Duration::days(days);

    let _ = sqlx::query(
        "INSERT INTO customers (id, email, name, company, created_at) VALUES (?1, ?2, ?3, 'Batch Procurement', ?4) ON CONFLICT(email) DO NOTHING"
    )
    .bind(&customer_id)
    .bind(&req.customer_email)
    .bind(&req.customer_name)
    .bind(now)
    .execute(&state.db)
    .await;

    let actual_cust_id = match sqlx::query("SELECT id FROM customers WHERE email = ?1")
        .bind(&req.customer_email)
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r.get::<String, _>("id"),
        Err(_) => customer_id,
    };

    let tier_enum = match req.tier.to_lowercase().as_str() {
        "laboratorio" => LicenseTier::Laboratorio,
        _ => LicenseTier::Peritaje,
    };

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let mut csv_summary = String::from("License_ID,Customer,Tier,Seats,Issued_At,Expires_At,Signature\n");

        for i in 1..=req.quantity {
            let lic_id = format!("LIC-BATCH-{}-{:04}", &Uuid::new_v4().to_string()[..8].to_uppercase(), i);
            let payload = LicensePayload {
                license_id: lic_id.clone(),
                customer_name: req.customer_name.clone(),
                customer_email: req.customer_email.clone(),
                tier: tier_enum.clone(),
                seats: req.seats_per_license,
                machine_fingerprint: None,
                allowed_machines: req.preassigned_fingerprints.clone().unwrap_or_default(),
                issued_at: now,
                expires_at: Some(expires_at),
                enabled_features: match req.tier.to_lowercase().as_str() {
                    "laboratorio" => LicenseTier::Laboratorio.default_features(),
                    _ => LicenseTier::Peritaje.default_features(),
                },
            };

            let signed = LicenseSigner::sign(payload, state.config.master_license_secret.as_bytes());
            let lic_json = serde_json::to_string_pretty(&signed).unwrap_or_default();

            let _ = sqlx::query(
                r#"
                INSERT INTO licenses (id, order_id, customer_id, tier, seats, issued_at, expires_at, signature, is_revoked, created_at)
                VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)
                "#
            )
            .bind(&lic_id)
            .bind(&actual_cust_id)
            .bind(req.tier.to_lowercase())
            .bind(req.seats_per_license as i64)
            .bind(now)
            .bind(expires_at)
            .bind(&signed.signature)
            .bind(now)
            .execute(&state.db)
            .await;

            let file_name = format!("{}.lic", lic_id);
            let _ = zip.start_file(&file_name, options);
            let _ = zip.write_all(lic_json.as_bytes());

            csv_summary.push_str(&format!(
                "{},\"{}\",{},{},{},{},{}\n",
                lic_id, req.customer_name, req.tier, req.seats_per_license, now.to_rfc3339(), expires_at.to_rfc3339(), signed.signature
            ));
        }

        let _ = zip.start_file("manifest_batch_summary.csv", options);
        let _ = zip.write_all(csv_summary.as_bytes());
        let _ = zip.finish();
    }

    let zip_bytes = buffer.into_inner();

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"batch_licenses_{}.zip\"", req.tier),
            ),
        ],
        zip_bytes,
    )
        .into_response()
}

pub async fn revoke_license(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<RevokeLicenseRequest>,
) -> Response {
    if !check_auth(&headers, &state).await {
        return (StatusCode::UNAUTHORIZED, "Acceso no autorizado").into_response();
    }

    let now = Utc::now();
    let crl_id = format!("CRL-{}", &Uuid::new_v4().to_string()[..8].to_uppercase());

    let update_res = sqlx::query(
        "UPDATE licenses SET is_revoked = 1, revocation_reason = ?1 WHERE id = ?2"
    )
    .bind(&req.reason)
    .bind(&req.license_id)
    .execute(&state.db)
    .await;

    if update_res.is_err() {
        return (StatusCode::NOT_FOUND, Json(json!({ "message": "Licencia no encontrada" }))).into_response();
    }

    let _ = sqlx::query(
        "INSERT INTO crl_revocations (id, license_id, machine_fingerprint, reason, revoked_at) VALUES (?1, ?2, ?3, ?4, ?5)"
    )
    .bind(&crl_id)
    .bind(&req.license_id)
    .bind(&req.machine_fingerprint)
    .bind(&req.reason)
    .bind(now)
    .execute(&state.db)
    .await;

    (StatusCode::OK, Json(json!({ "message": "Licencia revocada con éxito", "crl_id": crl_id }))).into_response()
}

pub async fn get_active_crl(
    State(state): State<Arc<AppState>>,
) -> Response {
    let revocations: Vec<CrlRevocation> = sqlx::query_as::<_, CrlRevocation>(
        "SELECT id, license_id, machine_fingerprint, reason, revoked_at FROM crl_revocations ORDER BY revoked_at DESC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "issuer": "ChatProof Pro CRL Authority",
        "generated_at": Utc::now(),
        "revocations": revocations
    }))).into_response()
}

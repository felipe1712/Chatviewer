use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Product {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub ed25519_public_key: Option<String>,
    #[serde(skip_serializing)]
    pub ed25519_private_key: Option<String>,
    pub allowed_tiers_json: String,
    pub metadata_json: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Customer {
    pub id: String,
    pub email: String,
    pub name: String,
    pub company: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: String,
    pub customer_id: String,
    pub product_id: Option<String>,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub tier: String,
    pub seats: i64,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub metadata_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LicenseRecord {
    pub id: String,
    pub order_id: Option<String>,
    pub customer_id: String,
    pub product_id: Option<String>,
    pub tier: String,
    pub seats: i64,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub signature: String,
    pub is_revoked: bool,
    pub revocation_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LicenseSeat {
    pub id: String,
    pub license_id: String,
    pub machine_fingerprint: String,
    pub dongle_id: Option<String>,
    pub label: Option<String>,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CrlRevocation {
    pub id: String,
    pub license_id: String,
    pub machine_fingerprint: Option<String>,
    pub reason: String,
    pub revoked_at: DateTime<Utc>,
}

// ==========================================
// DTOs (Data Transfer Objects)
// ==========================================

#[derive(Debug, Deserialize)]
pub struct CreateProductRequest {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub allowed_tiers: Option<Vec<String>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub allowed_tiers: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCheckoutRequest {
    pub email: String,
    pub name: String,
    pub company: Option<String>,
    pub product_slug: Option<String>, // 'chatproof-pro' por defecto
    pub tier: String,                 // 'caso', 'peritaje', 'laboratorio'
    pub billing_cycle: Option<String>,// 'monthly' o 'annual' o 'caso'
    pub provider: String,             // 'stripe' o 'manual'
    pub seats: Option<u32>,
    pub amount_override: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub checkout_url: String,
    pub order_id: String,
    pub session_id: String,
    pub provider: String,
    pub product_slug: String,
}

#[derive(Debug, Deserialize)]
pub struct AssignSeatRequest {
    pub machine_fingerprint: String,
    pub dongle_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchGenerateRequest {
    pub customer_name: String,
    pub customer_email: String,
    pub product_slug: Option<String>,
    pub tier: String,
    pub quantity: u32,
    pub seats_per_license: u32,
    pub valid_days: Option<i64>,
    pub preassigned_fingerprints: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct RevokeLicenseRequest {
    pub license_id: String,
    pub machine_fingerprint: Option<String>,
    pub reason: String,
}

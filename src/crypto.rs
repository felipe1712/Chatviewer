use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LicenseTier {
    #[serde(rename = "free", alias = "trial")]
    Free,
    #[serde(rename = "peritaje", alias = "professional")]
    Peritaje,
    #[serde(rename = "laboratorio", alias = "enterprise", alias = "team")]
    Laboratorio,
}

impl LicenseTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            LicenseTier::Free => "free",
            LicenseTier::Peritaje => "peritaje",
            LicenseTier::Laboratorio => "laboratorio",
        }
    }

    pub fn default_seats(&self) -> u32 {
        match self {
            LicenseTier::Free => 1,
            LicenseTier::Peritaje => 1,
            LicenseTier::Laboratorio => 5,
        }
    }

    pub fn default_features(&self) -> Vec<String> {
        match self {
            LicenseTier::Free => vec![
                "whatsapp_android".to_string(),
                "whatsapp_ios".to_string(),
                "sms_mms".to_string(),
                "telegram".to_string(),
                "rsmf_inspect".to_string(),
                "export_5_percent".to_string(),
            ],
            LicenseTier::Peritaje => vec![
                "whatsapp_android".to_string(),
                "whatsapp_ios".to_string(),
                "sms_mms".to_string(),
                "telegram".to_string(),
                "rsmf_inspect".to_string(),
                "rsmf_repair".to_string(),
                "dual_hash_audit".to_string(),
                "unlimited_export".to_string(),
            ],
            LicenseTier::Laboratorio => vec![
                "whatsapp_android".to_string(),
                "whatsapp_ios".to_string(),
                "sms_mms".to_string(),
                "telegram".to_string(),
                "rsmf_inspect".to_string(),
                "rsmf_repair".to_string(),
                "dual_hash_audit".to_string(),
                "unlimited_export".to_string(),
                "multi_seat_5".to_string(),
                "premium_support_sla".to_string(),
                "custom_forensic_branding".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LicensePayload {
    pub license_id: String,
    pub customer_name: String,
    pub customer_email: String,
    pub tier: LicenseTier,
    pub seats: u32,
    pub machine_fingerprint: Option<String>,
    #[serde(default)]
    pub allowed_machines: Vec<String>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub enabled_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedLicense {
    pub payload: LicensePayload,
    pub signature: String, // HMAC-SHA256 en Hex
}

pub struct LicenseSigner;

impl LicenseSigner {
    /// Firma criptográfica determinista compatible con ChatViewer Pro
    pub fn sign(payload: LicensePayload, secret: &[u8]) -> SignedLicense {
        let payload_json = serde_json::to_string(&payload)
            .expect("El payload de licencia siempre debe ser serializable a JSON");

        let mut mac = HmacSha256::new_from_slice(secret)
            .expect("HMAC-SHA256 admite claves de cualquier longitud");
        mac.update(payload_json.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        SignedLicense { payload, signature }
    }

    /// Verificación en tiempo constante
    pub fn verify(license: &SignedLicense, secret: &[u8]) -> bool {
        let payload_json = match serde_json::to_string(&license.payload) {
            Ok(j) => j,
            Err(_) => return false,
        };

        let mut mac = match HmacSha256::new_from_slice(secret) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(payload_json.as_bytes());
        let expected_sig = hex::encode(mac.finalize().into_bytes());

        expected_sig.as_bytes().ct_eq(license.signature.as_bytes()).unwrap_u8() == 1
    }
}

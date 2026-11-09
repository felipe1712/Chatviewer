use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub master_license_secret: String,
    pub stripe_secret_key: String,
    pub stripe_webhook_secret: String,
    pub stripe_success_url: String,
    pub stripe_cancel_url: String,
    pub brevo_api_key: String,
    pub brevo_sender_email: String,
    pub brevo_sender_name: String,
    pub admin_token: String,
    pub public_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8095);

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/licenses.db?mode=rwc".to_string());

        let master_license_secret = env::var("MASTER_LICENSE_SECRET")
            .unwrap_or_else(|_| "ChatProof-Pro-Forensic-Standalone-License-Root-Key-2026".to_string());

        let stripe_secret_key = env::var("STRIPE_SECRET_KEY")
            .unwrap_or_else(|_| "sk_test_chatproof_placeholder".to_string());

        let stripe_webhook_secret = env::var("STRIPE_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "whsec_test_chatproof_placeholder".to_string());

        let public_url = env::var("PUBLIC_URL")
            .unwrap_or_else(|_| "http://chatproof.pro".to_string());

        let stripe_success_url = env::var("STRIPE_SUCCESS_URL")
            .unwrap_or_else(|_| format!("{}/portal?session_id={{CHECKOUT_SESSION_ID}}", public_url));

        let stripe_cancel_url = env::var("STRIPE_CANCEL_URL")
            .unwrap_or_else(|_| format!("{}/#pricing", public_url));

        let brevo_api_key = env::var("BREVO_API_KEY")
            .unwrap_or_else(|_| "brevo_api_key_placeholder".to_string());

        let brevo_sender_email = env::var("BREVO_SENDER_EMAIL")
            .unwrap_or_else(|_| "licensing@chatproof.pro".to_string());

        let brevo_sender_name = env::var("BREVO_SENDER_NAME")
            .unwrap_or_else(|_| "ChatProof Pro".to_string());

        let admin_token = env::var("ADMIN_TOKEN")
            .unwrap_or_else(|_| "forensic-admin-master-key-2026".to_string());

        Self {
            port,
            database_url,
            master_license_secret,
            stripe_secret_key,
            stripe_webhook_secret,
            stripe_success_url,
            stripe_cancel_url,
            brevo_api_key,
            brevo_sender_email,
            brevo_sender_name,
            admin_token,
            public_url,
        }
    }
}

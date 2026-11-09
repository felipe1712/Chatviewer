use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::{Row, SqlitePool};
use subtle::ConstantTimeEq;
use tracing::info;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Derivación de clave segura estándar PBKDF2 con HMAC-SHA256
pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(password).expect("HMAC admite claves de cualquier longitud");
    mac.update(salt);
    mac.update(&1u32.to_be_bytes());
    let mut u = mac.finalize().into_bytes();
    let mut output = u;

    for _ in 1..iterations {
        let mut inner_mac = HmacSha256::new_from_slice(password).expect("HMAC");
        inner_mac.update(&u);
        u = inner_mac.finalize().into_bytes();
        for (o, b) in output.iter_mut().zip(u.iter()) {
            *o ^= b;
        }
    }

    let mut result = [0u8; 32];
    result.copy_from_slice(&output);
    result
}

/// Encripta y hashea una contraseña con salt aleatorio de 128 bits y 100,000 iteraciones
pub fn hash_password(password: &str) -> String {
    let salt = Uuid::new_v4().to_string().replace('-', "");
    let key = pbkdf2_hmac_sha256(password.as_bytes(), salt.as_bytes(), 100_000);
    format!("$pbkdf2-sha256$100000${}${}", salt, hex::encode(key))
}

/// Verifica en tiempo constante si la contraseña coincide con el hash almacenado
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parts: Vec<&str> = stored_hash.split('$').filter(|s| !s.is_empty()).collect();
    if parts.len() != 4 {
        return false;
    }
    if parts[0] != "pbkdf2-sha256" {
        return false;
    }
    let iterations: u32 = match parts[1].parse() {
        Ok(i) => i,
        Err(_) => return false,
    };
    let salt = parts[2];
    let expected_hex = parts[3];

    let computed = pbkdf2_hmac_sha256(password.as_bytes(), salt.as_bytes(), iterations);
    let computed_hex = hex::encode(computed);

    computed_hex.as_bytes().ct_eq(expected_hex.as_bytes()).into()
}

/// Asegura la existencia del superadministrador inicial en la base de datos
pub async fn bootstrap_superadmin(db: &SqlitePool, _master_admin_token: &str) {
    let count: i64 = sqlx::query("SELECT COUNT(*) as c FROM admin_users WHERE role = 'superadmin'")
        .fetch_one(db)
        .await
        .map(|r| r.get::<i64, _>("c"))
        .unwrap_or(0);

    if count == 0 {
        let admin_id = format!("USR-{}", Uuid::new_v4().to_string()[..8].to_uppercase());
        let default_email = std::env::var("SUPERADMIN_EMAIL")
            .unwrap_or_else(|_| "admin@causer.com.mx".to_string());
        let initial_pwd = std::env::var("SUPERADMIN_INITIAL_PASSWORD")
            .unwrap_or_else(|_| "CauserSuperAdmin2026!".to_string());
        
        let pwd_hash = hash_password(&initial_pwd);
        let now = Utc::now();

        let _ = sqlx::query(
            r#"
            INSERT INTO admin_users (id, email, name, password_hash, role, is_active, created_at, updated_at)
            VALUES (?1, ?2, 'Superadministrador Causer', ?3, 'superadmin', 1, ?4, ?4)
            "#
        )
        .bind(&admin_id)
        .bind(&default_email)
        .bind(&pwd_hash)
        .bind(now)
        .execute(db)
        .await;

        info!("============================================================");
        info!("[Seguridad] Superadministrador inicial inicializado en BD:");
        info!("  - Email:       {}", default_email);
        info!("  - Contraseña:  {}", initial_pwd);
        info!("  - Rol:         superadmin (Acceso Total)");
        info!("============================================================");
    }
}

/// Crea una sesión válida de 30 días para un usuario autenticado
pub async fn create_session(db: &SqlitePool, user_id: &str) -> Result<String, sqlx::Error> {
    let token = format!("csess_{}", Uuid::new_v4().to_string().replace('-', ""));
    let now = Utc::now();
    let expires_at = now + Duration::days(30);

    sqlx::query(
        "INSERT INTO admin_sessions (token, user_id, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)"
    )
    .bind(&token)
    .bind(user_id)
    .bind(expires_at)
    .bind(now)
    .execute(db)
    .await?;

    Ok(token)
}

/// Valida si un Bearer token es válido (sesión de usuario activa o token maestro)
pub async fn validate_token(db: &SqlitePool, master_token: &str, token: &str) -> Option<SessionUser> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    // 1. Token Maestro estático de configuración
    if token == master_token.trim() {
        return Some(SessionUser {
            id: "USR-MASTER-ROOT".to_string(),
            email: "root@causer.com.mx".to_string(),
            name: "Master Admin Root".to_string(),
            role: "superadmin".to_string(),
        });
    }

    // 2. Sesión activa en base de datos
    let now = Utc::now();
    let row = sqlx::query(
        r#"
        SELECT u.id, u.email, u.name, u.role, u.is_active
        FROM admin_sessions s
        JOIN admin_users u ON s.user_id = u.id
        WHERE s.token = ?1 AND s.expires_at > ?2 AND u.is_active = 1
        "#,
    )
    .bind(token)
    .bind(now)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()?;

    Some(SessionUser {
        id: row.get::<String, _>("id"),
        email: row.get::<String, _>("email"),
        name: row.get::<String, _>("name"),
        role: row.get::<String, _>("role"),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}

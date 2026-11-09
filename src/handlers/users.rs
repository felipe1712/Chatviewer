use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{create_session, hash_password, validate_token, verify_password, SessionUser};
use crate::handlers::checkout::AppState;

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers.get("authorization")?
        .to_str().ok()?
        .strip_prefix("Bearer ")
        .map(|s| s.trim())
}

async fn get_auth_user(headers: &HeaderMap, state: &AppState) -> Option<SessionUser> {
    let token = extract_bearer_token(headers)?;
    validate_token(&state.db, &state.config.admin_token, token).await
}

// ==========================================
// DTOs
// ==========================================

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub name: String,
    pub password: String,
    pub role: Option<String>, // 'admin', 'operator', 'superadmin'
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
    pub reset_password: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AdminUserDto {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
    pub is_active: bool,
    pub last_login_at: Option<chrono::DateTime<Utc>>,
    pub created_at: chrono::DateTime<Utc>,
}

// ==========================================
// Handlers
// ==========================================

/// Autenticación: Inicio de sesión de administradores
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let email = req.email.trim().to_lowercase();
    let row = sqlx::query(
        "SELECT id, email, name, password_hash, role, is_active FROM admin_users WHERE LOWER(email) = ?1"
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await;

    let user_row = match row {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "message": "Credenciales inválidas o usuario no registrado" })),
            )
                .into_response();
        }
    };

    let is_active: bool = user_row.get("is_active");
    if !is_active {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "La cuenta de usuario está desactivada. Contacte al superadministrador." })),
        )
            .into_response();
    }

    let user_id: String = user_row.get("id");
    let name: String = user_row.get("name");
    let stored_hash: String = user_row.get("password_hash");
    let role: String = user_row.get("role");

    if !verify_password(&req.password, &stored_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "message": "Contraseña incorrecta" })),
        )
            .into_response();
    }

    // Actualizar último login
    let now = Utc::now();
    let _ = sqlx::query("UPDATE admin_users SET last_login_at = ?1 WHERE id = ?2")
        .bind(now)
        .bind(&user_id)
        .execute(&state.db)
        .await;

    // Crear token de sesión
    match create_session(&state.db, &user_id).await {
        Ok(token) => (
            StatusCode::OK,
            Json(json!({
                "message": "Inicio de sesión exitoso",
                "token": token,
                "user": {
                    "id": user_id,
                    "name": name,
                    "email": email,
                    "role": role
                }
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "message": format!("Error creando sesión: {}", e) })),
        )
            .into_response(),
    }
}

/// Consulta de perfil del usuario logueado actualmente
pub async fn me_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    match get_auth_user(&headers, &state).await {
        Some(user) => (StatusCode::OK, Json(json!({ "user": user }))).into_response(),
        None => (StatusCode::UNAUTHORIZED, Json(json!({ "message": "No autenticado" }))).into_response(),
    }
}

/// Modificación de contraseña del usuario en la plataforma
pub async fn change_password_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> Response {
    let auth_user = match get_auth_user(&headers, &state).await {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({ "message": "No autenticado" }))).into_response(),
    };

    if auth_user.id == "USR-MASTER-ROOT" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "La cuenta Master Root se configura exclusivamente vía variable de entorno ADMIN_TOKEN" })),
        )
            .into_response();
    }

    let row = match sqlx::query("SELECT password_hash FROM admin_users WHERE id = ?1")
        .bind(&auth_user.id)
        .fetch_one(&state.db)
        .await
    {
        Ok(r) => r,
        Err(_) => return (StatusCode::NOT_FOUND, Json(json!({ "message": "Usuario no encontrado" }))).into_response(),
    };

    let current_hash: String = row.get("password_hash");
    if !verify_password(&req.current_password, &current_hash) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "La contraseña actual no coincide" })),
        )
            .into_response();
    }

    if req.new_password.trim().len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "La nueva contraseña debe tener al menos 8 caracteres" })),
        )
            .into_response();
    }

    let new_hash = hash_password(&req.new_password);
    let now = Utc::now();

    let _ = sqlx::query("UPDATE admin_users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(&new_hash)
        .bind(now)
        .bind(&auth_user.id)
        .execute(&state.db)
        .await;

    (
        StatusCode::OK,
        Json(json!({ "message": "Contraseña actualizada exitosamente" })),
    )
        .into_response()
}

/// Listar todos los administradores (Gestión de usuarios)
pub async fn list_users_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    let auth_user = match get_auth_user(&headers, &state).await {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({ "message": "No autenticado" }))).into_response(),
    };

    let users: Vec<AdminUserDto> = sqlx::query_as::<_, AdminUserDto>(
        "SELECT id, email, name, role, is_active, last_login_at, created_at FROM admin_users ORDER BY created_at ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    (StatusCode::OK, Json(json!({ "users": users, "requester_role": auth_user.role }))).into_response()
}

/// Crear un nuevo usuario en la plataforma
pub async fn create_user_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateUserRequest>,
) -> Response {
    let auth_user = match get_auth_user(&headers, &state).await {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({ "message": "No autenticado" }))).into_response(),
    };

    if auth_user.role != "superadmin" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Solo el Superadministrador puede registrar nuevos usuarios" })),
        )
            .into_response();
    }

    let email = req.email.trim().to_lowercase();
    if email.is_empty() || req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "El nombre y el correo electrónico son requeridos" })),
        )
            .into_response();
    }

    if req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "La contraseña debe tener mínimo 8 caracteres" })),
        )
            .into_response();
    }

    let id = format!("USR-{}", Uuid::new_v4().to_string()[..8].to_uppercase());
    let password_hash = hash_password(&req.password);
    let role = req.role.unwrap_or_else(|| "admin".to_string());
    let now = Utc::now();

    let res = sqlx::query(
        r#"
        INSERT INTO admin_users (id, email, name, password_hash, role, is_active, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)
        "#,
    )
    .bind(&id)
    .bind(&email)
    .bind(req.name.trim())
    .bind(&password_hash)
    .bind(&role)
    .bind(now)
    .execute(&state.db)
    .await;

    match res {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({
                "message": "Usuario creado exitosamente",
                "id": id,
                "email": email,
                "role": role
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(json!({ "message": format!("Error creando usuario (correo duplicado): {}", e) })),
        )
            .into_response(),
    }
}

/// Actualizar usuario (rol, nombre, estado o reset de contraseña)
pub async fn update_user_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Response {
    let auth_user = match get_auth_user(&headers, &state).await {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({ "message": "No autenticado" }))).into_response(),
    };

    if auth_user.role != "superadmin" && auth_user.id != user_id {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Privilegios insuficientes" })),
        )
            .into_response();
    }

    let now = Utc::now();

    if let Some(ref new_pwd) = req.reset_password {
        if new_pwd.len() >= 8 {
            let hash = hash_password(new_pwd);
            let _ = sqlx::query("UPDATE admin_users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(&hash)
                .bind(now)
                .bind(&user_id)
                .execute(&state.db)
                .await;
        }
    }

    if let Some(ref name) = req.name {
        let _ = sqlx::query("UPDATE admin_users SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(name.trim())
            .bind(now)
            .bind(&user_id)
            .execute(&state.db)
            .await;
    }

    if auth_user.role == "superadmin" {
        if let Some(ref role) = req.role {
            let _ = sqlx::query("UPDATE admin_users SET role = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(role)
                .bind(now)
                .bind(&user_id)
                .execute(&state.db)
                .await;
        }

        if let Some(is_active) = req.is_active {
            let _ = sqlx::query("UPDATE admin_users SET is_active = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(is_active)
                .bind(now)
                .bind(&user_id)
                .execute(&state.db)
                .await;
        }
    }

    (
        StatusCode::OK,
        Json(json!({ "message": "Usuario actualizado correctamente" })),
    )
        .into_response()
}

/// Eliminar usuario
pub async fn delete_user_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Response {
    let auth_user = match get_auth_user(&headers, &state).await {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({ "message": "No autenticado" }))).into_response(),
    };

    if auth_user.role != "superadmin" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "message": "Solo el Superadministrador puede eliminar usuarios" })),
        )
            .into_response();
    }

    if auth_user.id == user_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "No puedes eliminar tu propia cuenta de superadministrador" })),
        )
            .into_response();
    }

    let _ = sqlx::query("DELETE FROM admin_sessions WHERE user_id = ?1")
        .bind(&user_id)
        .execute(&state.db)
        .await;

    let _ = sqlx::query("DELETE FROM admin_users WHERE id = ?1")
        .bind(&user_id)
        .execute(&state.db)
        .await;

    (
        StatusCode::OK,
        Json(json!({ "message": "Usuario eliminado exitosamente" })),
    )
        .into_response()
}

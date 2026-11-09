use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::handlers::checkout::AppState;
use crate::models::{CreateProductRequest, Product, UpdateProductRequest};

async fn check_auth(headers: &HeaderMap, state: &AppState) -> bool {
    if let Some(auth) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            return crate::auth::validate_token(&state.db, &state.config.admin_token, token).await.is_some();
        }
    }
    false
}

/// Lista pública de productos activos y sus claves públicas de verificación
pub async fn public_list_products(
    State(state): State<Arc<AppState>>,
) -> Response {
    let products: Vec<Product> = sqlx::query_as::<_, Product>(
        "SELECT id, slug, name, description, ed25519_public_key, NULL as ed25519_private_key, allowed_tiers_json, metadata_json, is_active, created_at, updated_at FROM products WHERE is_active = 1 ORDER BY name ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    (StatusCode::OK, Json(json!({ "products": products }))).into_response()
}

/// Panel Admin: Listar todos los productos con estadísticas de licencias
pub async fn admin_list_products(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if !check_auth(&headers, &state).await {
        return (StatusCode::UNAUTHORIZED, "Acceso no autorizado").into_response();
    }

    let products: Vec<Product> = sqlx::query_as::<_, Product>(
        "SELECT id, slug, name, description, ed25519_public_key, ed25519_private_key, allowed_tiers_json, metadata_json, is_active, created_at, updated_at FROM products ORDER BY created_at ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Contabilizar licencias por producto
    let mut enriched = Vec::new();
    for p in products {
        let lic_count: i64 = sqlx::query("SELECT COUNT(*) as c FROM licenses WHERE product_id = ?1")
            .bind(&p.id)
            .fetch_one(&state.db)
            .await
            .map(|r| r.get::<i64, _>("c"))
            .unwrap_or(0);

        enriched.push(json!({
            "id": p.id,
            "slug": p.slug,
            "name": p.name,
            "description": p.description,
            "ed25519_public_key": p.ed25519_public_key,
            "allowed_tiers": serde_json::from_str::<serde_json::Value>(&p.allowed_tiers_json).unwrap_or(json!([])),
            "is_active": p.is_active,
            "licenses_count": lic_count,
            "created_at": p.created_at,
            "updated_at": p.updated_at
        }));
    }

    (StatusCode::OK, Json(json!({ "products": enriched }))).into_response()
}

/// Panel Admin: Registrar un nuevo producto en el catálogo
pub async fn admin_create_product(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateProductRequest>,
) -> Response {
    if !check_auth(&headers, &state).await {
        return (StatusCode::UNAUTHORIZED, "Acceso no autorizado").into_response();
    }

    let slug = req.slug.trim().to_lowercase().replace(' ', "-");
    if slug.is_empty() || req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "message": "El slug y el nombre del producto son requeridos" })),
        )
            .into_response();
    }

    let id = format!("PROD-{}", Uuid::new_v4().to_string()[..8].to_uppercase());
    let allowed_tiers = req.allowed_tiers.unwrap_or_else(|| {
        vec!["caso".to_string(), "peritaje".to_string(), "laboratorio".to_string()]
    });
    let allowed_tiers_json = serde_json::to_string(&allowed_tiers).unwrap_or_else(|_| "[]".to_string());
    let metadata_json = req.metadata.map(|m| m.to_string());
    let now = Utc::now();

    // Semilla criptográfica inicial para el producto
    let initial_key_seed = format!("causer-{}-secret-{}", slug, Uuid::new_v4());

    let res = sqlx::query(
        r#"
        INSERT INTO products (id, slug, name, description, ed25519_private_key, allowed_tiers_json, metadata_json, is_active, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?9)
        "#,
    )
    .bind(&id)
    .bind(&slug)
    .bind(&req.name.trim())
    .bind(&req.description)
    .bind(&initial_key_seed)
    .bind(&allowed_tiers_json)
    .bind(&metadata_json)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await;

    match res {
        Ok(_) => (
            StatusCode::CREATED,
            Json(json!({
                "message": "Producto registrado con éxito",
                "id": id,
                "slug": slug,
                "name": req.name
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(json!({ "message": format!("Error registrando producto (posible slug duplicado): {}", e) })),
        )
            .into_response(),
    }
}

/// Panel Admin: Actualizar configuración de un producto
pub async fn admin_update_product(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateProductRequest>,
) -> Response {
    if !check_auth(&headers, &state).await {
        return (StatusCode::UNAUTHORIZED, "Acceso no autorizado").into_response();
    }

    let existing: Option<Product> = sqlx::query_as::<_, Product>(
        "SELECT * FROM products WHERE id = ?1 OR slug = ?1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let existing = match existing {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, Json(json!({ "message": "Producto no encontrado" }))).into_response(),
    };

    let name = req.name.unwrap_or(existing.name);
    let description = req.description.or(existing.description);
    let is_active = req.is_active.unwrap_or(existing.is_active);
    let allowed_tiers_json = match req.allowed_tiers {
        Some(tiers) => serde_json::to_string(&tiers).unwrap_or(existing.allowed_tiers_json),
        None => existing.allowed_tiers_json,
    };
    let now = Utc::now();

    let _ = sqlx::query(
        r#"
        UPDATE products
        SET name = ?1, description = ?2, is_active = ?3, allowed_tiers_json = ?4, updated_at = ?5
        WHERE id = ?6
        "#,
    )
    .bind(&name)
    .bind(&description)
    .bind(is_active)
    .bind(&allowed_tiers_json)
    .bind(now)
    .bind(&existing.id)
    .execute(&state.db)
    .await;

    (StatusCode::OK, Json(json!({ "message": "Producto actualizado correctamente", "id": existing.id }))).into_response()
}

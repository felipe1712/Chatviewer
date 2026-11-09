mod config;
mod crypto;
mod email;
mod handlers;
mod models;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::fs;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info};

use crate::config::Config;
use crate::handlers::checkout::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Inicializar logger forense
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,license_server=debug".into()),
        )
        .init();

    // 2. Cargar configuración de entorno
    let config = Arc::new(Config::from_env());

    // Asegurar directorio data/
    let _ = fs::create_dir_all("data");
    let _ = fs::create_dir_all("/app/data");

    // 3. Inicializar SQLite y ejecutar migraciones
    info!("Conectando a base de datos de licencias: {}", config.database_url);
    let connect_opts = match SqliteConnectOptions::from_str(&config.database_url) {
        Ok(opts) => opts.create_if_missing(true),
        Err(e) => {
            error!("Error parseando DATABASE_URL ({}): {}", config.database_url, e);
            return Err(e.into());
        }
    };

    let db = match SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(connect_opts)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            error!("Error crítico conectando a SQLite en {}: {}", config.database_url, e);
            return Err(e.into());
        }
    };

    info!("Ejecutando migraciones de esquema de licencias...");
    if let Err(e) = sqlx::migrate!("./migrations").run(&db).await {
        error!("Error ejecutando migraciones de base de datos: {}", e);
        return Err(e.into());
    }

    let state = Arc::new(AppState {
        db,
        config: Arc::clone(&config),
    });

    // 4. Configurar CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 5. Configurar Enrutador Axum
    let app = Router::new()
        // --- SITIO PÚBLICO (LANDING PAGE & SEO) ---
        .route("/", get(handlers::landing::serve_hub))
        .route("/chatproof", get(handlers::landing::serve_landing))
        .route("/portal", get(handlers::landing::serve_portal))
        .route("/admin", get(handlers::landing::serve_admin))
        .route("/download/evaluation", get(handlers::landing::download_free_evaluation))
        .route("/download/sample-package", get(handlers::landing::download_sample_package))
        .route("/robots.txt", get(handlers::landing::serve_robots))
        .route("/sitemap.xml", get(handlers::landing::serve_sitemap))

        // --- PASARELA DE PAGO (STRIPE) ---
        .route("/api/checkout/create", post(handlers::checkout::create_checkout))
        .route("/api/webhooks/stripe", post(handlers::webhooks::handle_stripe_webhook))

        // --- PORTAL DE AUTOGESTIÓN DE CLIENTES ---
        .route("/api/portal/licenses", get(handlers::portal::list_portal_licenses))
        .route("/api/portal/license/:id/assign", post(handlers::portal::assign_seat))
        .route("/api/portal/license/:id/download", get(handlers::portal::download_license))

        // --- PANEL DE ADMINISTRACIÓN Y REVOCACIONES (CRL) ---
        .route("/api/admin/overview", get(handlers::admin::admin_overview))
        .route("/api/admin/batch", post(handlers::admin::batch_generate))
        .route("/api/admin/revoke", post(handlers::admin::revoke_license))
        .route("/api/crl/active", get(handlers::admin::get_active_crl))

        // --- CATÁLOGO MULTI-PRODUCTO (CAUSER ENGINE) ---
        .route("/api/public/products", get(handlers::products::public_list_products))
        .route("/api/admin/products", get(handlers::products::admin_list_products).post(handlers::products::admin_create_product))
        .route("/api/admin/products/:id", axum::routing::put(handlers::products::admin_update_product))

        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("============================================================");
    info!(" Causer Engine · Servidor Central (admin.causer.com.mx)");
    info!(" Escuchando en: http://{}", addr);
    info!(" - Landing pública:     http://localhost:{}", config.port);
    info!(" - Portal cliente:      http://localhost:{}/portal", config.port);
    info!(" - Panel administrador: http://localhost:{}/admin", config.port);
    info!("============================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

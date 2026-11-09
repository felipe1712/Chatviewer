mod config;
mod crypto;
mod email;
mod handlers;
mod models;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

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

    // 3. Inicializar SQLite y ejecutar migraciones
    info!("Conectando a base de datos de licencias: {}", config.database_url);
    let db = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    info!("Ejecutando migraciones de esquema de licencias...");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await?;

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
        .route("/", get(handlers::landing::serve_landing))
        .route("/portal", get(handlers::landing::serve_portal))
        .route("/admin", get(handlers::landing::serve_admin))
        .route("/download/evaluation", get(handlers::landing::download_free_evaluation))
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

        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("============================================================");
    info!(" Servidor Central de Licencias ChatProof Pro");
    info!(" Escuchando en: http://{}", addr);
    info!(" - Landing pública:     http://localhost:{}", config.port);
    info!(" - Portal cliente:      http://localhost:{}/portal", config.port);
    info!(" - Panel administrador: http://localhost:{}/admin", config.port);
    info!("============================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

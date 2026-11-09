use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use chrono::{Duration, Utc};
use std::io::{Cursor, Write};
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::crypto::{LicensePayload, LicenseSigner, LicenseTier};
use crate::handlers::checkout::AppState;

const LANDING_HTML: &str = include_str!("../../templates/landing.html");
const LANDING_EN_HTML: &str = include_str!("../../templates/landing_en.html");
const PORTAL_HTML: &str = include_str!("../../templates/portal.html");
const ADMIN_HTML: &str = include_str!("../../templates/admin.html");

const ROBOTS_TXT: &str = "User-agent: *\nAllow: /\nDisallow: /admin\nDisallow: /api/\n\nSitemap: https://chatproof.pro/sitemap.xml\n";

const SITEMAP_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n  <url>\n    <loc>https://chatproof.pro/</loc>\n    <lastmod>2026-11-09</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>1.0</priority>\n  </url>\n  <url>\n    <loc>https://chatproof.pro/en</loc>\n    <lastmod>2026-11-09</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>0.9</priority>\n  </url>\n  <url>\n    <loc>https://chatproof.pro/portal</loc>\n    <lastmod>2026-11-09</lastmod>\n    <changefreq>monthly</changefreq>\n    <priority>0.8</priority>\n  </url>\n</urlset>\n";

pub async fn serve_landing() -> Html<&'static str> {
    Html(LANDING_HTML)
}

pub async fn serve_landing_en() -> Html<&'static str> {
    Html(LANDING_EN_HTML)
}

pub async fn serve_portal() -> Html<&'static str> {
    Html(PORTAL_HTML)
}

pub async fn serve_admin() -> Html<&'static str> {
    Html(ADMIN_HTML)
}

pub async fn serve_robots() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        ROBOTS_TXT,
    )
        .into_response()
}

pub async fn serve_sitemap() -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        SITEMAP_XML,
    )
        .into_response()
}

/// Descarga de la versión de evaluación gratuita (Free 7 días)
pub async fn download_free_evaluation(
    State(state): State<Arc<AppState>>,
) -> Response {
    let now = Utc::now();
    let payload = LicensePayload {
        license_id: format!("FREE-EVAL-{}", &uuid::Uuid::new_v4().to_string()[..8].to_uppercase()),
        customer_name: "Evaluación Pericial (7 Días)".to_string(),
        customer_email: "evaluacion@chatproof.pro".to_string(),
        tier: LicenseTier::Free,
        seats: 1,
        machine_fingerprint: None,
        allowed_machines: Vec::new(),
        issued_at: now,
        expires_at: Some(now + Duration::days(7)),
        enabled_features: LicenseTier::Free.default_features(),
    };

    let signed = LicenseSigner::sign(payload, state.config.master_license_secret.as_bytes());
    let lic_json = serde_json::to_string_pretty(&signed).unwrap_or_default();

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"chatproof_free_trial.lic\"",
            ),
        ],
        lic_json,
    )
        .into_response()
}

/// Descarga del paquete pericial de entrega de ejemplo (con datos sintéticos)
pub async fn download_sample_package() -> Response {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 1. CERTIFICADO DE HASHES DUAL
        let cert_txt = r#"================================================================================
CHATPROOF PRO · CERTIFICADO FORENSE DE INTEGRIDAD CRIPTOGRÁFICA (MUESTRA SINTÉTICA)
Desarrollado por Causer Consulting · Principios ISO/IEC 27037:2012
================================================================================

EXPEDIENTE / CASO DE REFERENCIA : EXP-2026-F982-SINTETICO
FECHA Y HORA DE ADQUISICIÓN     : 2026-11-09 10:15:32 UTC
PERITO / EXAMINADOR             : Perito Informático Forense Certificado
HERRAMIENTA UTILIZADA           : ChatProof Pro v2.0 (Build x86_64 Air-Gapped)

1. EVIDENCIA DE ENTRADA (ORIGEN):
--------------------------------------------------------------------------------
Archivo Procesado : msgstore.db (WhatsApp Android DB)
Tamaño en Bytes   : 48,291,840 bytes
Hash SHA-256      : e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
Hash MD5          : d41d8cd98f00b204e9800998ecf8427e
Estado Integridad : ÍNTEGRO (Coincide al 100% con imagen forense raw original)

2. TOTALES DE CONTROL Y TRAZABILIDAD:
--------------------------------------------------------------------------------
Total Mensajes en Base Peritada : 14,250
Mensajes Procesados e Indexados : 14,250
Adjuntos Multimedia Detectados  : 842 (Imágenes: 710, Audios: 112, Docs: 20)
Adjuntos Validados por Hash     : 842
Discrepancias o Desfases        : 0

3. PAQUETES FORENSES DE SALIDA:
--------------------------------------------------------------------------------
Formato Principal : RSMF 2.0 (Relativity Short Message Format RFC-5322 Compliant)
Formato Judicial  : Reporte Pericial PDF Foliado con Índices Cronológicos UTC
Visor Pericial    : Expediente HTML Interactivo Autónomo (100% Offline)
Bitácora Auditoría: Cadena de bloques criptográfica encadenada (Hash Chaining)

CERTIFICACIÓN:
Se certifica que el procesamiento de la presente muestra sintética se llevó a cabo
con estricto apego a las directrices de preservación, repetibilidad técnica y 
auditoría forense para sustentar la admisibilidad probatoria en sede judicial.
"#;
        let _ = zip.start_file("01_CERTIFICADO_HASHES.txt", options);
        let _ = zip.write_all(cert_txt.as_bytes());

        // 2. CADENA DE CUSTODIA Y AUDITORIA ENCADENADA (JSON)
        let audit_json = serde_json::json!({
            "case_id": "EXP-2026-F982-SINTETICO",
            "standard_guidelines": "ISO/IEC 27037:2012 Identification, Collection, Acquisition & Preservation",
            "developer": "Causer Consulting",
            "audit_chain": [
                {
                    "step_index": 1,
                    "event": "INGESTA_EVIDENCIA_DIGITAL",
                    "timestamp_utc": "2026-11-09T10:15:32.104Z",
                    "input_source": "WhatsApp msgstore.db (Android Raw Extracted)",
                    "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                    "previous_hash": "0000000000000000000000000000000000000000000000000000000000000000",
                    "block_hash": "7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b"
                },
                {
                    "step_index": 2,
                    "event": "NORMALIZACION_CONVERSACIONES",
                    "timestamp_utc": "2026-11-09T10:15:45.320Z",
                    "total_messages": 14250,
                    "participants_identified": 18,
                    "previous_hash": "7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b",
                    "block_hash": "9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e3d2c1b0a9f8e"
                },
                {
                    "step_index": 3,
                    "event": "EXPORTACION_RSMF_Y_PDF",
                    "timestamp_utc": "2026-11-09T10:16:02.890Z",
                    "output_format": "RSMF 2.0 & Judicial PDF",
                    "rsmf_files_generated": 24,
                    "previous_hash": "9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e3d2c1b0a9f8e",
                    "block_hash": "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b"
                }
            ],
            "chain_validation": "VALIDATED_CONTINUOUS_INTEGRITY"
        });
        let _ = zip.start_file("02_CADENA_CUSTODIA_AUDIT_LOG.json", options);
        let _ = zip.write_all(serde_json::to_string_pretty(&audit_json).unwrap_or_default().as_bytes());

        // 3. MUESTRA DE MANIFEST RSMF 2.0
        let rsmf_manifest = serde_json::json!({
            "rsmf": {
                "version": "2.0.0",
                "applicant": "ChatProof Pro by Causer Consulting"
            },
            "participants": [
                {
                    "id": "custodio_01",
                    "display": "Lic. Carlos Mendoza (Custodio)",
                    "email": "+525512345678@s.whatsapp.net"
                },
                {
                    "id": "interlocutor_02",
                    "display": "Ing. Sofía Morales",
                    "email": "+525598765432@s.whatsapp.net"
                }
            ],
            "conversations": [
                {
                    "id": "conv-001",
                    "display": "Coordinación Operativa y Financiera",
                    "events": [
                        {
                            "id": "msg-001",
                            "type": "message",
                            "participant": "custodio_01",
                            "timestamp": "2026-10-14T14:22:05Z",
                            "body": "Envío adjunto el balance financiero solicitado para la junta de directores."
                        },
                        {
                            "id": "msg-002",
                            "type": "message",
                            "participant": "interlocutor_02",
                            "timestamp": "2026-10-14T14:23:18Z",
                            "body": "Recibido de conformidad, comienzo la revisión de los anexos."
                        }
                    ]
                }
            ]
        });
        let _ = zip.start_file("03_MUESTRA_RSMF_MANIFEST.json", options);
        let _ = zip.write_all(serde_json::to_string_pretty(&rsmf_manifest).unwrap_or_default().as_bytes());

        // 4. GUIA DE ENTREGA JUDICIAL (README)
        let readme = r#"ESTRUCTURA TÍPICA DE ENTREGA DE UN PERITAJE CON CHATPROOF PRO
=============================================================================
Al procesar una investigación o caso pericial con ChatProof Pro, el perito
obtiene una carpeta estructurada lista para su presentación judicial:

1. 01_CERTIFICADO_HASHES.txt / .pdf
   Contiene el acta de verificación criptográfica dual (SHA-256 y MD5) de
   los archivos fuente peritados y de los archivos de salida generados.

2. 02_CADENA_CUSTODIA_AUDIT_LOG.json
   Bitácora inalterable con encadenamiento de hashes bloque por bloque (Hash Chaining),
   demostrando la ausencia de manipulación temporal o de contenido.

3. 03_MUESTRA_RSMF_MANIFEST.json y archivos .eml
   Paquetes con estricta adherencia a la especificación RSMF 2.0 para su ingestión
   en Relativity o archivo probatorio.

4. REPORTE JUDICIAL EN FORMATO PDF
   Documento formalmente paginado y foliado, con cronología UTC, participantes y
   marcas de custodia listo para ser firmado por el perito e incorporado al expediente.

Respaldado por Causer Consulting · https://chatproof.pro · contacto@causer.com.mx
"#;
        let _ = zip.start_file("LEEME_ESTRUCTURA_ENTREGA.txt", options);
        let _ = zip.write_all(readme.as_bytes());

        let _ = zip.finish();
    }

    let zip_bytes = buffer.into_inner();
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"paquete_ejemplo_chatproof_pro.zip\"",
            ),
        ],
        zip_bytes,
    )
        .into_response()
}

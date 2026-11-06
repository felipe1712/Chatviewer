use base64::Engine;
use serde_json::json;
use tracing::{info, warn};

use crate::config::Config;

pub struct EmailService;

impl EmailService {
    /// Envía la licencia firmada y las instrucciones al cliente mediante Brevo (Sendinblue)
    pub async fn send_license_email(
        config: &Config,
        to_email: &str,
        to_name: &str,
        license_id: &str,
        tier: &str,
        lic_json_content: &str,
    ) -> Result<bool, String> {
        if config.brevo_api_key.starts_with("brevo_api_key_placeholder") {
            info!(
                "[Brevo Mock] Clave de Brevo en modo placeholder. Simulando envío de correo a {} con licencia {}",
                to_email, license_id
            );
            return Ok(true);
        }

        let client = reqwest::Client::new();
        let base64_content = base64::engine::general_purpose::STANDARD.encode(lic_json_content.as_bytes());

        let html_content = format!(
            r#"
            <div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0 auto; color: #1e293b; padding: 20px;">
                <div style="text-align: center; border-bottom: 2px solid #2563eb; padding-bottom: 20px; margin-bottom: 20px;">
                    <h2 style="color: #1e40af; margin: 0;">ChatViewer <span style="background: #2563eb; color: #fff; padding: 2px 8px; border-radius: 4px; font-size: 14px;">PRO</span></h2>
                    <p style="color: #64748b; font-size: 14px; margin: 5px 0 0;">Certificación Procesal y Análisis Forense Digital</p>
                </div>

                <p>Estimado(a) <strong>{}</strong>,</p>
                <p>Tu orden de compra ha sido confirmada exitosamente. Adjunto a este correo encontrarás tu archivo oficial de licencia criptográfica (<strong>chatviewer_license.lic</strong>).</p>

                <div style="background: #f1f5f9; border-left: 4px solid #2563eb; padding: 15px; margin: 20px 0; border-radius: 4px;">
                    <div><strong>ID de Licencia:</strong> <span style="font-family: monospace;">{}</span></div>
                    <div><strong>Nivel / Tier:</strong> <span style="text-transform: uppercase;">{}</span></div>
                    <div><strong>Validez:</strong> 100% Offline (Compatible con Laboratorios Aislados)</div>
                </div>

                <h3>¿Cómo activar tu licencia en ChatViewer Pro?</h3>
                <ol style="line-height: 1.6;">
                    <li>Abre <strong>ChatViewer Pro</strong> en tu estación de trabajo.</li>
                    <li>Haz clic en el botón <strong>Licencia</strong> en la barra superior.</li>
                    <li>Abre el archivo adjunto <code>chatviewer_license.lic</code> con un bloc de notas, copia el contenido JSON y pégalo en la ventana de activación.</li>
                    <li>Presiona <strong>Instalar y Validar Licencia</strong>.</li>
                </ol>

                <p style="margin-top: 25px;">Si necesitas gestionar tus estaciones o reasignar un puesto, puedes acceder a tu <a href="{}/portal?email={}" style="color: #2563eb; font-weight: bold;">Portal de Autogestión</a>.</p>

                <div style="border-top: 1px solid #e2e8f0; margin-top: 30px; padding-top: 15px; font-size: 12px; color: #94a3b8; text-align: center;">
                    ChatViewer Pro © 2026 · Cumple con los estándares procesales ISO/IEC 27037 y RSMF 2.0.
                </div>
            </div>
            "#,
            to_name, license_id, tier, config.public_url, to_email
        );

        let payload = json!({
            "sender": {
                "name": &config.brevo_sender_name,
                "email": &config.brevo_sender_email
            },
            "to": [
                {
                    "email": to_email,
                    "name": to_name
                }
            ],
            "subject": format!("Tu Licencia Comercial de ChatViewer Pro ({})", license_id),
            "htmlContent": html_content,
            "attachment": [
                {
                    "name": "chatviewer_license.lic",
                    "content": base64_content
                }
            ]
        });

        match client
            .post("https://api.brevo.com/v3/smtp/email")
            .header("api-key", &config.brevo_api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!("Correo de licencia enviado exitosamente con Brevo a {}", to_email);
                    Ok(true)
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    warn!("Error respuesta API de Brevo: {}", err_text);
                    Err(err_text)
                }
            }
            Err(e) => {
                warn!("Fallo de conexión enviando correo con Brevo: {}", e);
                Err(e.to_string())
            }
        }
    }
}

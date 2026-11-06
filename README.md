# ChatViewer Pro · Servidor Comercial de Licencias y Sitio Web

Repositorio del sitio público, portal de clientes, panel de administración y servidor de emisión de licencias criptográficas para **ChatViewer Pro** ([http://chatviewer.pro](http://chatviewer.pro)).

---

## 🚀 Arquitectura
- **Backend:** Rust con Axum 0.7 y Tokio.
- **Base de Datos:** SQLite con SQLx (migraciones automáticas).
- **Frontend Embebido:** HTML5 / Bootstrap 5.3 (compilado directamente dentro del binario con `include_str!`).
- **Pagos:** Integración con Stripe Checkout y Webhooks.
- **Emails:** Envío de licencias transaccionales mediante Brevo API.
- **SEO:** Metadatos avanzados, Schema.org (JSON-LD con `SoftwareApplication`, `FAQPage`, `Organization`), `/robots.txt` y `/sitemap.xml`.

---

## 🛠️ Despliegue en Ubuntu Server

### Opción 1: Con Docker Compose (Recomendada)
```bash
# 1. Clonar el repositorio
git clone https://github.com/felipe1712/Chatviewer.git /var/www/chatviewer
cd /var/www/chatviewer

# 2. Configurar variables de entorno
cp .env.example .env
nano .env

# 3. Construir y levantar
docker compose up -d --build
```

### Opción 2: Como Servicio Systemd Nativo
```bash
# 1. Instalar Rust y dependencias
sudo apt update && sudo apt install -y pkg-config libssl-dev libsqlite3-dev build-essential
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# 2. Clonar y compilar
git clone https://github.com/felipe1712/Chatviewer.git /var/www/chatviewer
cd /var/www/chatviewer
cp .env.example .env
cargo build --release

# 3. Configurar servicio systemd
sudo cp deploy/chatviewer.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable chatviewer
sudo systemctl start chatviewer
```

---

## 🌐 Configuración de Nginx (Puerto 80)
Copia el archivo `deploy/nginx.conf` a `/etc/nginx/sites-available/chatviewer.pro`:
```bash
sudo cp deploy/nginx.conf /etc/nginx/sites-available/chatviewer.pro
sudo ln -s /etc/nginx/sites-available/chatviewer.pro /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

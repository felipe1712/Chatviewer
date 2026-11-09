# Causer License Engine · Plataforma Central de Licenciamiento y Facturación

> **Visión:** Motor independiente de gestión de licencias, membresías, productos, cobros y correo transaccional (alternativa soberana e in-house a servicios como Keygen.sh o Cryptlex), diseñado en Rust para máxima velocidad, seguridad criptográfica y soporte 100% offline / Air-Gapped.

---

## 🎯 Objetivo del Proyecto Independiente
Centralizar la monetización, activación y gobernanza de software para todo el catálogo de productos de **Causer Consulting** (y potencialmente terceros):
1. **Multi-Producto:** Administrar múltiples herramientas (escritorio, CLI, servicios web) desde un solo backend.
2. **Criptografía Asimétrica (Ed25519):** Firma con clave privada en el servidor y validación offline con clave pública embebida en los ejecutables del cliente.
3. **Pasarela de Cobro Centralizada:** Integración única con Stripe (pagos únicos, por caso, membresías recurrentes, cupones).
4. **Despacho Automatizado de Licencias:** Integración con Brevo para entrega inmediata de archivos criptográficos `.lic`.
5. **Portal de Autogestión del Cliente ("Causer Hub"):** Panel donde el comprador visualiza todos sus productos adquiridos, descarga sus licencias y administra sus estaciones de trabajo (hardware fingerprints).
6. **Lista de Revocación de Certificados (CRL):** Endpoint público para invalidar licencias comprometidas o reembolsadas.

---

## 🏗️ Hoja de Ruta de Evolución (Roadmap a Repositorio Propio)

### Fase 1: Esquema Multi-Producto (Base de Datos)
- [ ] Tabla `products`:
  - `id` (UUID)
  - `slug` (ej: `chatproof`, `mobile-extractor`, `network-auditor`)
  - `name` (Nombre comercial)
  - `ed25519_public_key` / `ed25519_private_key`
  - `allowed_tiers` (JSON: planes disponibles)
  - `is_active` (booleano)
- [ ] Relacionar `orders.product_id` y `licenses.product_id`.

### Fase 2: Firma Criptográfica Asimétrica (Ed25519)
- [ ] Migrar de HMAC-SHA256 (clave simétrica compartida) a **Ed25519**:
  - El servidor firma digitalmente el payload JSON de la licencia con su llave privada.
  - El software en el cliente solo almacena la llave pública para verificar la firma `ed25519_dalek`.
  - **Ventaja de seguridad:** Aunque el usuario aplique ingeniería inversa al binario del aplicativo, **nunca** podrá extraer la llave de firma ni generar licencias falsas.

### Fase 3: API Gateway & SDKs para Clientes
- [ ] Endpoints RESTful estandarizados:
  - `POST /v1/checkout/create` (iniciar compra de cualquier producto)
  - `POST /v1/licenses/validate` (validación opcional online)
  - `GET /v1/licenses/crl/:product_slug` (descarga de CRL por producto)
  - `POST /v1/portal/session` (acceso al hub de clientes)
- [ ] SDKs ultraligeros de validación:
  - Rust crate: `causer-licensing-client`
  - Python wheel: `causer_license`
  - TypeScript / Electron wrapper

### Fase 4: Desacoplamiento a Repositorio Propio
- [ ] Crear el repositorio independiente: `github.com/felipe1712/causer-license-engine.git`.
- [ ] Migrar el código base limpio sin archivos específicos de la landing de un producto.
- [ ] Montar como servicio central permanente en `https://licensing.causer.com.mx` (o `auth.causer.com.mx`).

---

## 🛡️ Comparativa Técnica frente a Soluciones Comerciales

| Característica | Keygen.sh / Cryptlex | Causer License Engine |
| :--- | :--- | :--- |
| **Costo Mensual** | \$200 – \$1,500 USD / mes | \$5 USD / mes (VPS propio) |
| **Soberanía de Datos** | Servidores en EE.UU. / UE | 100% Soberano (infraestructura propia) |
| **Soporte Air-Gapped / Offline** | Restringido o plan Enterprise | Nativo de primer nivel (Ed25519) |
| **Tecnología** | Ruby / Node / Go | Rust + Axum + SQLite/Postgres |
| **Consumo de Memoria** | ~300 - 800 MB | ~15 - 30 MB RAM |
| **Control de Códigos Fuente** | Propietario cerrado | Código fuente total propiedad de Causer |

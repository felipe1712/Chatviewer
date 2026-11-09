-- Migración para soporte Multi-Producto en Causer License Engine
-- Fase 1: Tabla de productos y relación con órdenes y licencias

CREATE TABLE IF NOT EXISTS products (
    id TEXT PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    ed25519_public_key TEXT,
    ed25519_private_key TEXT,
    allowed_tiers_json TEXT NOT NULL DEFAULT '["caso","peritaje","laboratorio"]',
    metadata_json TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Insertar producto raíz (ChatProof Pro) para compatibilidad
INSERT OR IGNORE INTO products (id, slug, name, description, allowed_tiers_json, is_active)
VALUES (
    'PROD-CHATPROOF-PRO',
    'chatproof-pro',
    'ChatProof Pro',
    'Visor Forense y Conversor Oficial RSMF 2.0 y Reportes PDF Judiciales',
    '["caso","peritaje","laboratorio"]',
    1
);

-- Vincular products con orders y licenses
ALTER TABLE orders ADD COLUMN product_id TEXT REFERENCES products(id) DEFAULT 'PROD-CHATPROOF-PRO';
ALTER TABLE licenses ADD COLUMN product_id TEXT REFERENCES products(id) DEFAULT 'PROD-CHATPROOF-PRO';

CREATE INDEX IF NOT EXISTS idx_products_slug ON products(slug);
CREATE INDEX IF NOT EXISTS idx_orders_product ON orders(product_id);
CREATE INDEX IF NOT EXISTS idx_licenses_product ON licenses(product_id);

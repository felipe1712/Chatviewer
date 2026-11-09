-- Migración inicial para el Servidor de Licencias de ChatProof Pro

CREATE TABLE IF NOT EXISTS customers (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    company TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL REFERENCES customers(id),
    provider TEXT NOT NULL, -- 'stripe', 'manual'
    provider_session_id TEXT UNIQUE,
    tier TEXT NOT NULL,     -- 'free', 'peritaje', 'laboratorio'
    seats INTEGER NOT NULL DEFAULT 1,
    amount_cents INTEGER NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'paid', 'failed', 'refunded'
    metadata_json TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS licenses (
    id TEXT PRIMARY KEY,
    order_id TEXT REFERENCES orders(id),
    customer_id TEXT NOT NULL REFERENCES customers(id),
    tier TEXT NOT NULL, -- 'free', 'peritaje', 'laboratorio'
    seats INTEGER NOT NULL DEFAULT 1,
    issued_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME,
    signature TEXT NOT NULL,
    is_revoked BOOLEAN NOT NULL DEFAULT 0,
    revocation_reason TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS license_seats (
    id TEXT PRIMARY KEY,
    license_id TEXT NOT NULL REFERENCES licenses(id),
    machine_fingerprint TEXT NOT NULL,
    dongle_id TEXT,
    label TEXT,
    assigned_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(license_id, machine_fingerprint)
);

CREATE TABLE IF NOT EXISTS crl_revocations (
    id TEXT PRIMARY KEY,
    license_id TEXT NOT NULL,
    machine_fingerprint TEXT,
    reason TEXT NOT NULL,
    revoked_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS webhook_logs (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'processed',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_orders_customer ON orders(customer_id);
CREATE INDEX IF NOT EXISTS idx_licenses_customer ON licenses(customer_id);
CREATE INDEX IF NOT EXISTS idx_license_seats_lic ON license_seats(license_id);
CREATE INDEX IF NOT EXISTS idx_crl_license ON crl_revocations(license_id);

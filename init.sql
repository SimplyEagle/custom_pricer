-- Database Schema for Historical TF2 Listings
CREATE TABLE IF NOT EXISTS historical_listings (
    id SERIAL PRIMARY KEY,
    sku VARCHAR(255) NOT NULL,
    intent VARCHAR(10) NOT NULL,
    keys INT DEFAULT 0,
    metal REAL DEFAULT 0.0,
    price_total_metal REAL NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_historical_sku_time ON historical_listings(sku, created_at);

CREATE TABLE IF NOT EXISTS historical_rollups (
    sku VARCHAR(255) NOT NULL,
    intent VARCHAR(10) NOT NULL, 
    record_date DATE NOT NULL,
    median_price REAL NOT NULL,
    volume INT NOT NULL,
    PRIMARY KEY (sku, intent, record_date)
);

-- Indexes are also safely gated
CREATE INDEX IF NOT EXISTS idx_rollup_sku_intent_date ON historical_rollups(sku, intent, record_date);
CREATE INDEX IF NOT EXISTS idx_rollup_sku_date ON historical_rollups(sku, record_date);
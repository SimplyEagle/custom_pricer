-- Table for our compressed, long-term historical data
CREATE TABLE IF NOT EXISTS historical_rollups (
    sku VARCHAR(255) NOT NULL,
    record_date DATE NOT NULL,
    median_price REAL NOT NULL,
    volume INT NOT NULL,
    PRIMARY KEY (sku, record_date)
);

-- Index for fast long-term lookups
CREATE INDEX idx_rollup_sku_date ON historical_rollups(sku, record_date);
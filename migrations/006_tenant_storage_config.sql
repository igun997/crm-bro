-- migrations/006_tenant_storage_config.sql
CREATE TABLE IF NOT EXISTS tenant_storage_configs (
    id                INT AUTO_INCREMENT PRIMARY KEY,
    tenant_id         INT NOT NULL UNIQUE,
    endpoint          VARCHAR(512) NOT NULL,
    region            VARCHAR(64) NOT NULL DEFAULT 'auto',
    access_key_id     VARCHAR(256) NOT NULL,
    secret_access_key VARCHAR(512) NOT NULL,
    bucket            VARCHAR(256) NOT NULL,
    public_base_url   VARCHAR(512) DEFAULT NULL,
    is_active         TINYINT(1) NOT NULL DEFAULT 1,
    created_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id)
);

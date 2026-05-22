-- MVP1 additive multi-tenant schema

CREATE TABLE IF NOT EXISTS tenants (
    id INT PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(120) NOT NULL UNIQUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS users (
    id INT PRIMARY KEY AUTO_INCREMENT,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    tenant_id INT NULL,
    is_superadmin BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_users_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE SET NULL
);

DROP PROCEDURE IF EXISTS add_column_if_missing;
DROP PROCEDURE IF EXISTS add_fk_if_missing;
DROP PROCEDURE IF EXISTS add_index_if_missing;
DROP PROCEDURE IF EXISTS drop_fk_on_column_if_exists;

DELIMITER //
CREATE PROCEDURE add_column_if_missing(IN table_name_in VARCHAR(64), IN column_name_in VARCHAR(64), IN column_def_in TEXT)
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
          AND COLUMN_NAME = column_name_in
    ) THEN
        SET @sql = CONCAT('ALTER TABLE `', table_name_in, '` ADD COLUMN ', column_def_in);
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//

CREATE PROCEDURE add_fk_if_missing(IN table_name_in VARCHAR(64), IN constraint_name_in VARCHAR(64), IN fk_def_in TEXT)
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.TABLE_CONSTRAINTS
        WHERE CONSTRAINT_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
          AND CONSTRAINT_NAME = constraint_name_in
    ) THEN
        SET @sql = CONCAT('ALTER TABLE `', table_name_in, '` ADD CONSTRAINT `', constraint_name_in, '` ', fk_def_in);
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//

CREATE PROCEDURE add_index_if_missing(IN table_name_in VARCHAR(64), IN index_name_in VARCHAR(64), IN index_def_in TEXT)
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
    ) AND NOT EXISTS (
        SELECT 1 FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
          AND INDEX_NAME = index_name_in
    ) THEN
        SET @sql = CONCAT('ALTER TABLE `', table_name_in, '` ADD INDEX `', index_name_in, '` ', index_def_in);
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//

CREATE PROCEDURE drop_fk_on_column_if_exists(IN table_name_in VARCHAR(64), IN column_name_in VARCHAR(64))
BEGIN
    DECLARE fk_name VARCHAR(64);

    SELECT CONSTRAINT_NAME INTO fk_name
    FROM information_schema.KEY_COLUMN_USAGE
    WHERE CONSTRAINT_SCHEMA = DATABASE()
      AND TABLE_NAME = table_name_in
      AND COLUMN_NAME = column_name_in
      AND REFERENCED_TABLE_NAME IS NOT NULL
    LIMIT 1;

    IF fk_name IS NOT NULL THEN
        SET @sql = CONCAT('ALTER TABLE `', table_name_in, '` DROP FOREIGN KEY `', fk_name, '`');
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//
DELIMITER ;

CALL add_column_if_missing('users', 'tenant_id', 'tenant_id INT NULL');
CALL add_column_if_missing('users', 'is_superadmin', 'is_superadmin BOOLEAN NOT NULL DEFAULT FALSE');
CALL add_column_if_missing('users', 'is_active', 'is_active BOOLEAN NOT NULL DEFAULT TRUE');
CALL drop_fk_on_column_if_exists('users', 'tenant_id');
CALL add_fk_if_missing('users', 'fk_users_tenant', 'FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE SET NULL');

CREATE TABLE IF NOT EXISTS permissions (
    id INT PRIMARY KEY AUTO_INCREMENT,
    code VARCHAR(120) NOT NULL UNIQUE,
    description VARCHAR(255),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS roles (
    id INT PRIMARY KEY AUTO_INCREMENT,
    tenant_id INT NULL,
    name VARCHAR(120) NOT NULL,
    description VARCHAR(255),
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_roles_tenant_name (tenant_id, name),
    CONSTRAINT fk_roles_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id INT NOT NULL,
    permission_id INT NOT NULL,
    PRIMARY KEY (role_id, permission_id),
    CONSTRAINT fk_role_permissions_role FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    CONSTRAINT fk_role_permissions_permission FOREIGN KEY (permission_id) REFERENCES permissions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id INT NOT NULL,
    role_id INT NOT NULL,
    PRIMARY KEY (user_id, role_id),
    CONSTRAINT fk_user_roles_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_user_roles_role FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS tenant_whatsapp_accounts (
    id INT PRIMARY KEY AUTO_INCREMENT,
    tenant_id INT NOT NULL,
    phone_number_id VARCHAR(128) NOT NULL UNIQUE,
    business_account_id VARCHAR(128) NOT NULL,
    display_phone_number VARCHAR(32),
    access_token TEXT NOT NULL,
    verify_token VARCHAR(255) NOT NULL,
    api_version VARCHAR(32) NOT NULL DEFAULT 'v25.0',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_tenant_whatsapp_accounts_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
);

CREATE TABLE IF NOT EXISTS contacts (
    id INT PRIMARY KEY AUTO_INCREMENT,
    tenant_id INT NOT NULL,
    phone VARCHAR(32) NOT NULL,
    name VARCHAR(255),
    email VARCHAR(255),
    company VARCHAR(255),
    notes TEXT,
    owner_user_id INT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_contacts_tenant_phone (tenant_id, phone),
    CONSTRAINT fk_contacts_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    CONSTRAINT fk_contacts_owner_user FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS tags (
    id INT PRIMARY KEY AUTO_INCREMENT,
    tenant_id INT NOT NULL,
    name VARCHAR(80) NOT NULL,
    color VARCHAR(20),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY uq_tags_tenant_name (tenant_id, name),
    CONSTRAINT fk_tags_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id)
);

CREATE TABLE IF NOT EXISTS contact_tags (
    contact_id INT NOT NULL,
    tag_id INT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (contact_id, tag_id),
    CONSTRAINT fk_contact_tags_contact FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE CASCADE,
    CONSTRAINT fk_contact_tags_tag FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CALL add_column_if_missing('conversations', 'tenant_id', 'tenant_id INT NULL');
CALL add_column_if_missing('conversations', 'contact_id', 'contact_id INT NULL');
CALL add_column_if_missing('conversations', 'whatsapp_account_id', 'whatsapp_account_id INT NULL');

CALL add_column_if_missing('messages', 'tenant_id', 'tenant_id INT NULL');
CALL add_column_if_missing('messages', 'contact_id', 'contact_id INT NULL');
CALL add_column_if_missing('messages', 'storage_key', 'storage_key VARCHAR(1024) NULL');
CALL add_column_if_missing('messages', 'original_filename', 'original_filename VARCHAR(255) NULL');
CALL add_column_if_missing('messages', 'size_bytes', 'size_bytes BIGINT NULL');

INSERT INTO tenants (name, slug, is_active)
SELECT 'Default Tenant', 'default', TRUE
WHERE NOT EXISTS (SELECT 1 FROM tenants WHERE slug = 'default');

INSERT INTO contacts (tenant_id, phone, name)
SELECT t.id, c.contact_phone, NULLIF(c.contact_name, '')
FROM conversations c
JOIN tenants t ON t.slug = 'default'
WHERE c.contact_phone IS NOT NULL
ON DUPLICATE KEY UPDATE
    name = COALESCE(contacts.name, VALUES(name));

UPDATE conversations c
JOIN tenants t ON t.slug = 'default'
LEFT JOIN contacts ct ON ct.tenant_id = t.id AND ct.phone = c.contact_phone
SET c.tenant_id = COALESCE(c.tenant_id, t.id),
    c.contact_id = COALESCE(c.contact_id, ct.id)
WHERE c.tenant_id IS NULL OR c.contact_id IS NULL;

UPDATE messages m
JOIN conversations c ON c.id = m.conversation_id
SET m.tenant_id = COALESCE(m.tenant_id, c.tenant_id),
    m.contact_id = COALESCE(m.contact_id, c.contact_id)
WHERE m.tenant_id IS NULL OR m.contact_id IS NULL;

CALL drop_fk_on_column_if_exists('conversations', 'tenant_id');
CALL drop_fk_on_column_if_exists('conversations', 'contact_id');
CALL drop_fk_on_column_if_exists('conversations', 'whatsapp_account_id');
CALL add_fk_if_missing('conversations', 'fk_conversations_tenant', 'FOREIGN KEY (tenant_id) REFERENCES tenants(id)');
CALL add_fk_if_missing('conversations', 'fk_conversations_contact', 'FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE SET NULL');
CALL add_fk_if_missing('conversations', 'fk_conversations_whatsapp_account', 'FOREIGN KEY (whatsapp_account_id) REFERENCES tenant_whatsapp_accounts(id) ON DELETE SET NULL');

CALL drop_fk_on_column_if_exists('contacts', 'owner_user_id');
CALL add_fk_if_missing('contacts', 'fk_contacts_owner_user', 'FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE SET NULL');

CALL drop_fk_on_column_if_exists('messages', 'tenant_id');
CALL drop_fk_on_column_if_exists('messages', 'contact_id');
CALL add_fk_if_missing('messages', 'fk_messages_tenant', 'FOREIGN KEY (tenant_id) REFERENCES tenants(id)');
CALL add_fk_if_missing('messages', 'fk_messages_contact', 'FOREIGN KEY (contact_id) REFERENCES contacts(id) ON DELETE SET NULL');

CREATE TABLE IF NOT EXISTS outbox_messages (
    id INT PRIMARY KEY AUTO_INCREMENT,
    tenant_id INT NOT NULL,
    message_id INT NOT NULL,
    kind VARCHAR(50) NOT NULL,
    payload_json JSON NOT NULL,
    status ENUM('pending','processing','done','failed') NOT NULL DEFAULT 'pending',
    attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    next_attempt_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_outbox_messages_tenant FOREIGN KEY (tenant_id) REFERENCES tenants(id),
    CONSTRAINT fk_outbox_messages_message FOREIGN KEY (message_id) REFERENCES messages(id)
);

CALL add_index_if_missing('users', 'idx_users_tenant_active', '(tenant_id, is_active)');
CALL add_index_if_missing('roles', 'idx_roles_tenant', '(tenant_id)');
CALL add_index_if_missing('tenant_whatsapp_accounts', 'idx_twa_tenant_active', '(tenant_id, is_active)');
CALL add_index_if_missing('contacts', 'idx_contacts_tenant_owner', '(tenant_id, owner_user_id)');
CALL add_index_if_missing('conversations', 'idx_conversations_tenant_last_message', '(tenant_id, last_message_at)');
CALL add_index_if_missing('conversations', 'idx_conversations_tenant_contact', '(tenant_id, contact_id)');
CALL add_index_if_missing('messages', 'idx_messages_tenant_conversation_timestamp', '(tenant_id, conversation_id, timestamp)');
CALL add_index_if_missing('messages', 'idx_messages_tenant_contact_timestamp', '(tenant_id, contact_id, timestamp)');
CALL add_index_if_missing('outbox_messages', 'idx_outbox_polling', '(status, next_attempt_at, id)');
CALL add_index_if_missing('outbox_messages', 'idx_outbox_tenant_status', '(tenant_id, status, next_attempt_at)');

DROP PROCEDURE IF EXISTS add_column_if_missing;
DROP PROCEDURE IF EXISTS add_fk_if_missing;
DROP PROCEDURE IF EXISTS add_index_if_missing;
DROP PROCEDURE IF EXISTS drop_fk_on_column_if_exists;

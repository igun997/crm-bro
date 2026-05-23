-- Make conversations compatible with tenant-scoped contacts.

DROP PROCEDURE IF EXISTS drop_index_if_exists;
DROP PROCEDURE IF EXISTS add_unique_index_if_missing;

DELIMITER //
CREATE PROCEDURE drop_index_if_exists(IN table_name_in VARCHAR(64), IN index_name_in VARCHAR(64))
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
          AND INDEX_NAME = index_name_in
    ) THEN
        SET @sql = CONCAT('ALTER TABLE `', table_name_in, '` DROP INDEX `', index_name_in, '`');
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//

CREATE PROCEDURE add_unique_index_if_missing(IN table_name_in VARCHAR(64), IN index_name_in VARCHAR(64), IN index_def_in TEXT)
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
          AND INDEX_NAME = index_name_in
    ) THEN
        SET @sql = CONCAT('ALTER TABLE `', table_name_in, '` ADD UNIQUE KEY `', index_name_in, '` ', index_def_in);
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//
DELIMITER ;

CALL drop_index_if_exists('conversations', 'contact_phone');
CALL add_unique_index_if_missing('conversations', 'uq_conversations_tenant_contact', '(tenant_id, contact_id)');
CALL add_unique_index_if_missing('conversations', 'uq_conversations_tenant_phone', '(tenant_id, contact_phone)');

DROP PROCEDURE IF EXISTS add_unique_index_if_missing;
DROP PROCEDURE IF EXISTS drop_index_if_exists;

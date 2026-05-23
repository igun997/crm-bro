-- Support worker recovery scan for stale processing jobs.

DROP PROCEDURE IF EXISTS add_index_if_missing;

DELIMITER //
CREATE PROCEDURE add_index_if_missing(IN table_name_in VARCHAR(64), IN index_name_in VARCHAR(64), IN index_def_in TEXT)
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_NAME = table_name_in
          AND INDEX_NAME = index_name_in
    ) THEN
        SET @sql = CONCAT('CREATE INDEX `', index_name_in, '` ON `', table_name_in, '` ', index_def_in);
        PREPARE stmt FROM @sql;
        EXECUTE stmt;
        DEALLOCATE PREPARE stmt;
    END IF;
END//
DELIMITER ;

CALL add_index_if_missing('outbox_messages', 'idx_outbox_processing_stale', '(status, updated_at, id)');

DROP PROCEDURE IF EXISTS add_index_if_missing;

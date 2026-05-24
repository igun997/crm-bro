-- Allow async outbox-created messages to stay queued until worker sends them.

ALTER TABLE messages
    MODIFY COLUMN status ENUM('queued', 'sent', 'delivered', 'read', 'failed', 'received') DEFAULT 'received';

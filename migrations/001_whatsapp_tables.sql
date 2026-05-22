-- WhatsApp PoC: conversations + messages tables

CREATE TABLE IF NOT EXISTS conversations (
    id INT PRIMARY KEY AUTO_INCREMENT,
    contact_phone VARCHAR(20) NOT NULL UNIQUE,
    contact_name VARCHAR(255),
    last_message_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS messages (
    id INT PRIMARY KEY AUTO_INCREMENT,
    conversation_id INT NOT NULL,
    wa_message_id VARCHAR(128) UNIQUE,
    direction ENUM('inbound', 'outbound') NOT NULL,
    msg_type ENUM('text', 'template', 'image', 'document', 'audio', 'video') NOT NULL,
    body TEXT,
    media_url VARCHAR(512),
    media_mime VARCHAR(128),
    template_name VARCHAR(255),
    status ENUM('sent', 'delivered', 'read', 'failed', 'received') DEFAULT 'received',
    timestamp DATETIME NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);

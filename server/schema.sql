-- MindMap cloud. MariaDB / MySQL, utf8mb4, InnoDB.
-- Apply with: npm run setup

CREATE TABLE IF NOT EXISTS users (
  id INT UNSIGNED NOT NULL AUTO_INCREMENT,
  username VARCHAR(64) NOT NULL,
  password_hash VARCHAR(255) NOT NULL,
  created_at BIGINT NOT NULL,
  PRIMARY KEY (id),
  UNIQUE KEY users_username (username)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS sessions (
  token CHAR(64) NOT NULL,
  user_id INT UNSIGNED NOT NULL,
  expires_at BIGINT NOT NULL,
  PRIMARY KEY (token),
  KEY sessions_user (user_id),
  CONSTRAINT sessions_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS documents (
  user_id INT UNSIGNED NOT NULL,
  revision BIGINT UNSIGNED NOT NULL,
  content_hash CHAR(64) NOT NULL,
  library_json LONGTEXT NOT NULL,
  updated_at BIGINT NOT NULL,
  PRIMARY KEY (user_id),
  CONSTRAINT documents_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS backups (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  user_id INT UNSIGNED NOT NULL,
  revision BIGINT UNSIGNED NOT NULL,
  label VARCHAR(160) NOT NULL,
  content_hash CHAR(64) NOT NULL,
  library_json LONGTEXT NOT NULL,
  created_at BIGINT NOT NULL,
  PRIMARY KEY (id),
  KEY backups_user_created (user_id, created_at),
  CONSTRAINT backups_user_fk FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS login_attempts (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  username VARCHAR(64) NOT NULL,
  ip VARCHAR(64) NOT NULL,
  attempted_at BIGINT NOT NULL,
  PRIMARY KEY (id),
  KEY login_attempts_user (username, attempted_at),
  KEY login_attempts_ip (ip, attempted_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

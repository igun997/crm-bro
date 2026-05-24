use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub server_host: String,
    pub server_port: u16,
    pub app_base_url: String,
    pub storage_backend: String,
    pub storage_local_dir: String,
    pub r2_endpoint: Option<String>,
    pub r2_access_key_id: Option<String>,
    pub r2_secret_access_key: Option<String>,
    pub r2_bucket: Option<String>,
    pub r2_public_base_url: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();
        Self {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: std::env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            server_host: std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: std::env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("SERVER_PORT must be a number"),
            app_base_url: std::env::var("APP_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            storage_backend: std::env::var("STORAGE_BACKEND")
                .unwrap_or_else(|_| "local".to_string()),
            storage_local_dir: std::env::var("STORAGE_LOCAL_DIR")
                .unwrap_or_else(|_| "media".to_string()),
            r2_endpoint: std::env::var("R2_ENDPOINT").ok(),
            r2_access_key_id: std::env::var("R2_ACCESS_KEY_ID").ok(),
            r2_secret_access_key: std::env::var("R2_SECRET_ACCESS_KEY").ok(),
            r2_bucket: std::env::var("R2_BUCKET").ok(),
            r2_public_base_url: std::env::var("R2_PUBLIC_BASE_URL").ok(),
        }
    }
}

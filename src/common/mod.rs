pub mod auth;
pub mod config;
pub mod error;
pub mod middleware;

#[cfg(test)]
mod tests {
    use crate::common::{auth, config, error, middleware};

    #[test]
    fn shared_infrastructure_is_reexported_under_common() {
        let _claims_builder = auth::build_claims;
        let _config_loader = config::AppConfig::from_env;
        let _token_validator = middleware::validate_token;
        let _ok_response = error::ok::<()>;
    }
}

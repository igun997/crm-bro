#[test]
fn new_layers_are_exported() {
    let _ = crm_bro::application::auth::require_permission;
    let _ = crm_bro::infrastructure::config::AppConfig::from_env;
}

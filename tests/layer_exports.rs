#[test]
fn new_layers_are_exported() {
    let _ = std::any::type_name::<crm_bro::application::auth::Marker>();
    let _ = std::any::type_name::<crm_bro::infrastructure::config::Marker>();
}

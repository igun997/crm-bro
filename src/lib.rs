pub mod api;
pub mod auth;
pub mod common;
pub mod config;
pub mod domain;
pub mod middleware;
pub mod models;
pub mod rbac;
pub mod response;
pub mod routes;
pub mod storage;
pub mod whatsapp;
pub mod ws;

#[cfg(test)]
mod ddd_skeleton_tests {
    use crate::{api, common, domain};

    #[test]
    fn exposes_ddd_module_roots() {
        let _ = std::any::type_name::<
            fn() -> (
                api::routes::RoutesMarker,
                common::error::ErrorMarker,
                domain::storage::StorageMarker,
            ),
        >();
    }
}

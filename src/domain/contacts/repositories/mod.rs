pub mod contact_repository;
pub mod sea_orm_contact_repository;

pub use contact_repository::{ContactRepository, Pagination};
pub use sea_orm_contact_repository::SeaOrmContactRepository;

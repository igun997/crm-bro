use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: i32,
    pub tenant_id: Option<i32>,
    pub is_superadmin: bool,
    pub permissions: HashSet<String>,
}

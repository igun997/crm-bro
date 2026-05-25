pub mod entities;
pub mod errors;

#[cfg(test)]
mod tests {
    use super::entities::User;

    #[test]
    fn new_user_rejects_empty_email() {
        let result = User::new(Some(7), "   ", "Ada Lovelace", "hash");

        assert!(result.is_err());
    }

    #[test]
    fn new_user_normalizes_email() {
        let user =
            User::new(Some(7), "  ADA@EXAMPLE.COM  ", "Ada Lovelace", "hash").expect("valid user");

        assert_eq!(user.email(), "ada@example.com");
    }
}

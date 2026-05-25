pub mod contacts;

#[cfg(test)]
mod tests {
    use super::contacts::ContactResponse;
    use crate::domain::contacts::Contact;

    #[test]
    fn contact_response_from_domain_contact_uses_empty_tags() {
        let contact = Contact::new(1, "Jane".into(), "+62 899-692-6184".into()).unwrap();

        let response = ContactResponse::from(contact);

        assert_eq!(response.tenant_id, 1);
        assert_eq!(response.phone, "628996926184");
        assert_eq!(response.name.as_deref(), Some("Jane"));
        assert!(response.tags.is_empty());
    }
}

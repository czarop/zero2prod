use crate::domain::{SubscriberEmail, SubscriberName};

// A struct to store email and username
pub struct NewSubscriber {
    pub email: SubscriberEmail,
    pub name: SubscriberName,
}

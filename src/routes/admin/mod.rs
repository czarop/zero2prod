mod dashboard;
pub use dashboard::{admin_dashboard, get_username};

mod password;
pub use password::*;

mod logout;
pub use logout::log_out;

mod newsletter;
pub use newsletter::*;

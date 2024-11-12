use crate::helpers::{assert_is_redirect_to, spawn_app};

#[tokio::test]
async fn an_error_flash_message_is_set_on_failure() {
    // Arrange
    let app = spawn_app().await;
    // Act
    let login_body = serde_json::json!({
        "username": "random_username",
        "password": "random_password",
    });
    let response = app.post_login(&login_body).await;

    assert_is_redirect_to(&response, "/login");

    // Act pt 2 - follow the redirect - there should be a cookie
    // and an Auth Failed message
    let html_page = app.get_login_html().await;
    assert!(html_page.contains(r#"<p><i>Authentication failed</i></p>"#));

    // Act part 3 - reload the login page - the cookie should be gone
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains(r#"Authentication failed"#));
}

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

    // look for a header cookie with _flash
    let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();

    //Assert
    assert_eq!(flash_cookie.value(), "Authentication failed");
}

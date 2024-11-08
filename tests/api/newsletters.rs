use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers(){
    // Arrange
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // No request should be firest to postmark
        .mount(&app.email_server)
        .await;

    // Act - a sketch of a newsletter
    let newsletter_request_body = serde_json::json!({
        "title" : "Newsletter Title",
        "content" : {
            "text" : "Newsletter body as plain text",
            "html" : "<p>Newsletter body as HTML</p>,"
        }
    });

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.address))
        .json(&newsletter_request_body)
        .send()
        .await.expect("Failed to execute request.");

    // Assert
    assert_eq!(response.status().as_u16(), 200);
    // Mock verifies on Drop that we haven't sent the newsletter email
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks{
    let body = "name=le%20guin&email=tgslocombe%40outlook.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        // mount a server that will be dropped (and shut down) after the fn ends
        // this means it won't get confused with the other mock used in the
        // main test fn
        .mount_as_scoped(&app.email_server) 
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    // inspect the requests received by the mock Postmark server
    // retrieve the confirmation link and return it
    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    return app.get_confirmation_links(&email_request)

}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    
    // now click the confirmation link
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    
}

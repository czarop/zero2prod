use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// Generate some post requests and send to server
#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app = spawn_app().await;

    // a list of post request data. The data is specified as
    // tuple of strings, key value pair e.g name=Tom
    // non-unicode chars are encoded by % sign followed by code
    // e.g. space is %20 and @ is %40

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    // generate and send the post requests
    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(invalid_body.into()).await;

        // check as expected
        assert_eq!(
            400,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

// a valid post request
#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // Arrange
    let app = spawn_app().await;
    // A valid request
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;

    // Assert we are getting an OK back from the server
    assert_eq!(200, response.status().as_u16());

    // grab the first entry in the database

    /*
    The query! macro returns an anonymous record type: a struct definition is
    generated at compile-time after having verified that the query is valid, with
    a member for each column on the result!!
    */
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

// a valid post request
#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    // Arrange
    let app = spawn_app().await;
    // A valid request
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // Act
    app.post_subscriptions(body.into()).await;

    // Assert
    // grab the first entry in the database

    /*
    The query! macro returns an anonymous record type: a struct definition is
    generated at compile-time after having verified that the query is valid, with
    a member for each column on the result!!
    */
    let saved = sqlx::query!("SELECT email, name, status FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    // at this point it should be pending confirmation
    assert_eq!(saved.status, "pending_confirmation");
}

// a test for troublesome inputs
#[tokio::test]
async fn subscribe_returns_a_200_when_fields_are_present_but_empty() {
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = app.post_subscriptions(body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 OK when the payload wasd {}",
            description
        )
    }
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    let app = spawn_app().await;

    // set up the conditions to check
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // a valid request
    let req = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    app.post_subscriptions(req.into()).await;

    // Mock does the assertions when the fn ends
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    // Arrange
    let app = spawn_app().await;
    // A valid request
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // Act
    app.post_subscriptions(body.into()).await;

    // Assert
    // Get the first intercepted request
    let email_request = &app.email_server.received_requests().await.unwrap()[0];

    let confirmation_links = app.get_confirmation_links(&email_request);

    // The two links should be identical
    assert_eq!(confirmation_links.html, confirmation_links.plain_text);
}

// use std::{println as info, println as warn};

#[tokio::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    // sabotage the db
    sqlx::query("ALTER TABLE subscription_tokens DROP COLUMN subscription_token;")
        .execute(&app.db_pool)
        .await
        .unwrap();

    let response = app.post_subscriptions(body.into()).await;

    let status = response.status().as_u16();

    // cannot get their implementation to print the log on windows
    // so forced to print the error message
    // call cargo t subscribe_fails_if_there_is_a_fatal_database_error -- --show-output
    //println!("{:?}", &response.text().await.unwrap());

    // add to .cargo/.config
    // [env]
    // RUST_LOG="sqlx=error,info"
    // TEST_LOG="true"

    // try on linux:
    // export RUST_LOG="sqlx=error,info"
    // export TEST_LOG=true
    // cargo t subscribe_fails_if_there_is_a_fatal_database_error | bunyan

    assert_eq!(status, 500);
}

use crate::helpers::spawn_app;

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
async fn subscribe_returns_a_200_when_data_is_valid() {
    // Arrange
    let app = spawn_app().await;
    // A valid request
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
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

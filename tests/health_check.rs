use reqwest::Client;
use sqlx::{Connection, PgConnection};
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup;

// checks:
// the health check is exposed at /health_check;
// the health check is behind a GET method;
// the health check always returns a 200;
// the health check’s response has no body.
#[tokio::test] // for an async test
async fn health_check_works() {
    // spawn app starts the server as async task, also returns the bound address
    let address = spawn_app();

    // generate an http request sender
    let client = Client::new();

    let response = client
        .get(format!("{}/health_check", &address))
        .send()
        .await
        .expect("Failed to execute request"); // deal with errors

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

// there is no await call - so this fn does not need to be async
// don't propogate errors here - as only for testing - crash the program
fn spawn_app() -> String {
    // we want a random available port
    // specifying port 0 gives a random available port assigned by the OS
    // but we need to know which port it is so we can send requests to it
    // create a TcpListener to track which port is assigned for the server to bind
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port");

    // get the port - we have to do this before passing listner below, as it is moved
    let port = listener.local_addr().unwrap().port();

    // create the server
    let server = startup::run(listener).expect("Failed to launch Server");
    // launch the server as a background / non-blocking task
    let _ = tokio::spawn(server);
    // note spawn will drop all tasks when the tokio runtime is ended - so the
    // server will shut down when the test completes

    // return the bound address to the calling fn
    format!("http://127.0.0.1:{}", port)
}

// Generate some post requests and send to server
#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app_address = spawn_app();
    let client = Client::new();

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
        let response = client
            .post(&format!("{}/subscriptions", &app_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request");

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
    let app_address = spawn_app();
    // get the database settings from file loaded into a struct
    let configuration = get_configuration().expect("Failed to read configuration.yaml");
    // produce the required string fromt he settings to connect to the db
    let connection_string = configuration.database.connection_string();

    // the 'Connection' trait must be in scope for us to invoke
    // PgCOnnection::connect - it is not a method of the struct
    // connect to the database
    let mut connection = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to Postgres");

    // set up the request sender
    let client = reqwest::Client::new();
    // A valid request
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");
    // Assert we are getting an OK back from the server
    assert_eq!(200, response.status().as_u16());

    // grab the first entry in the database

    /*
    The query! macro returns an anonymous record type: a struct definition is
    generated at compile-time after having verified that the query is valid, with
    a member for each column on the result!!
    */
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&mut connection)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

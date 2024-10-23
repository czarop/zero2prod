use reqwest::Client;
use secrecy::Secret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use std::sync::LazyLock;
use uuid::Uuid;
use zero2prod::configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup;
use zero2prod::telemetry;

// checks:
// the health check is exposed at /health_check;
// the health check is behind a GET method;
// the health check always returns a 200;
// the health check’s response has no body.
#[tokio::test] // for an async test
async fn health_check_works() {
    // spawn app starts the server as async task, also returns the bound address
    let app_data = spawn_app().await;

    // generate an http request sender
    let client = Client::new();

    let response = client
        .get(format!("{}/health_check", &app_data.address))
        .send()
        .await
        .expect("Failed to execute request"); // deal with errors

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

// Ensure that the `tracing` stack is only initialised once using `LazyLock`
static TRACING: LazyLock<()> = LazyLock::new(|| {
    // if an env variable, TEST_LOG, is set - print log messages to std:io:stdout, otherwise bin messgaes
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber =
            telemetry::get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        telemetry::init_subscriber(subscriber);
    } else {
        let subscriber =
            telemetry::get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        telemetry::init_subscriber(subscriber);
    };
});

// a struct to hold the data relating to the app generation
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool, // connection to the db - a pool of connections for async queries
}

// don't propogate errors here - as only for testing - crash the program
async fn spawn_app() -> TestApp {
    //first set up telemetry spans
    // The first time `initialize` is invoked the code in `TRACING` is executed.
    // All other invocations will instead skip execution.
    LazyLock::force(&TRACING);

    // we want a random available port
    // specifying port 0 gives a random available port assigned by the OS
    // but we need to know which port it is so we can send requests to it
    // create a TcpListener to track which port is assigned for the server to bind
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port");

    // get the port - we have to do this before passing listner below, as it is moved
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    // generate the db connection - for this we need a connection string
    // produced in our configuration mod. Here we make it mut as we're going to bodge the
    // db name for testing purposes
    let mut configuration =
        configuration::get_configuration().expect("Failed to read configuration.");
    // give the db a random name so we're not interfering with our actual db
    configuration.database.database_name = Uuid::new_v4().to_string();

    // we need a connection to our fake db, just like in the real app we will use a pool
    // of connections, which are wrapped in Arc pointers
    // we now build a new db from scratch - just for the test!
    let connection_pool = configure_database(&configuration.database).await;

    // Build a new email client
    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address.");
    let timeout = std::time::Duration::from_millis(200);
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.auth_token,
        timeout,
    );

    // create the server - clone the connection pool as it is an Arc pointer,
    // this essentially passes a ref
    let server = startup::run(listener, connection_pool.clone(), email_client)
        .expect("Failed to launch Server");
    // launch the server as a background / non-blocking task
    let _ = tokio::spawn(server);
    // note spawn will drop all tasks when the tokio runtime is ended - so the
    // server will shut down when the test completes

    // return data to calling fn in our data struct
    TestApp {
        address: address,
        db_pool: connection_pool,
    }
}

pub async fn configure_database(config: &configuration::DatabaseSettings) -> PgPool {
    // create a test database
    let maintenance_settings = configuration::DatabaseSettings {
        database_name: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Secret::new("password".to_string()),
        ..config.clone()
    };

    let mut connection = PgConnection::connect_with(&maintenance_settings.connection_options())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // Migrate database
    let connection_pool = PgPool::connect_with(config.connection_options())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations") // same as sqlx-cli migrate run in our bash script
        .run(&connection_pool)
        .await
        .expect("Failed to migrate db");

    connection_pool
}

// Generate some post requests and send to server
#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    let app_data = spawn_app().await;
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
            .post(&format!("{}/subscriptions", &app_data.address))
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
    let app_data = spawn_app().await;
    // set up the request sender
    let client = reqwest::Client::new();

    // A valid request
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app_data.address))
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
        .fetch_one(&app_data.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

// a test for troublesome inputs
#[tokio::test]
async fn subscribe_returns_a_200_when_fields_are_present_but_empty() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return a 400 OK when the payload wasd {}",
            description
        )
    }
}

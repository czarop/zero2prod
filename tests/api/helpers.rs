use secrecy::Secret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::sync::LazyLock;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::configuration;
use zero2prod::startup;
use zero2prod::{startup::get_connection_pool, telemetry};

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
    pub email_server: MockServer, // a fake email server - we will check if emails are sent and what they contain
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

// don't propogate errors here - as only for testing - crash the program
pub async fn spawn_app() -> TestApp {
    //first set up telemetry spans
    // The first time `initialize` is invoked the code in `TRACING` is executed.
    // All other invocations will instead skip execution.
    LazyLock::force(&TRACING);

    // Launch a mock server to stand in for Postmark's API
    let email_server = MockServer::start().await;

    // generate the db connection - for this we need a connection string
    // produced in our configuration mod. Here we make it mut as we're going to bodge the
    // db name for testing purposes
    // Randomise configuration to ensure test isolation
    let configuration = {
        let mut c = configuration::get_configuration().expect("Failed to read configuration.");
        // Use a different database for each test case
        c.database.database_name = Uuid::new_v4().to_string();
        // Use a random OS port
        c.application.port = 0;
        // Use the mock server as email API
        c.email_client.base_url = email_server.uri();
        c
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    // Launch the application as a background task
    let application = startup::Application::build(configuration.clone())
        .await
        .expect("Failed to build application.");

    let application_port = application.port();

    let _ = tokio::spawn(application.run_until_stopped());

    let address = format!("http://localhost:{}", application_port);

    TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database),
        email_server,
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

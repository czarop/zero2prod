use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
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
    pub port: u16,                // we store the port of the app locally for testing purposes
    pub test_user: TestUser,
    pub api_client: reqwest::Client, // the http request client
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    ///Extract the confirmation links from the email body
    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        // get the body of the request
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        // a closure to find links - we use it below
        let get_link = |s: &str| {
            // linkify finds links...(!)
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();

            assert_eq!(links.len(), 1);

            // get the first (and only) link
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();

            // check it's a local address - not smth random on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");

            // re-write to include the port - only required for local
            confirmation_link.set_port(Some(self.port)).unwrap();

            confirmation_link
        };

        // get the html and plain text links - passed into the closure above
        let html = get_link(body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(body["TextBody"].as_str().unwrap());

        ConfirmationLinks { html, plain_text }
    }

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        let username = &self.test_user.username;
        let password = &self.test_user.password;

        self.api_client
            .post(&format!("{}/newsletters", &self.address))
            .basic_auth(username, Some(password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    // body is a generic type that can be deserialised
    pub async fn post_login<Body>(&self, body: &Body) -> reqwest::Response
    where
        Body: serde::Serialize,
    {
        self.api_client
            .post(&format!("{}/login", &self.address))
            .form(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub async fn get_login_html(&self) -> String {
        self.api_client
            .get(&format!("{}/login", &self.address))
            .send()
            .await
            .expect("Fail to execute request.")
            .text() // decodes the response body in full
            .await
            .unwrap()
    }

    pub async fn get_admin_dashboard_html(&self) -> String {
        self.get_admin_dashboard().await.text().await.unwrap()
    }

    pub async fn get_admin_dashboard(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/admin/dashboard", &self.address))
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

// a fake user of the API
pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}
impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }
    // store myself in the user_id db
    async fn store(&self, pool: &PgPool) {
        // allow us to hash in the same way when verfying
        let salt = SaltString::generate(&mut rand::thread_rng());

        // Match parameters of the default password
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();

        sqlx::query!(
            "INSERT INTO users (user_id, username, password_hash)
        VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash,
        )
        .execute(pool)
        .await
        .expect("Failed to store test user.");
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

    tokio::spawn(application.run_until_stopped());

    let address = format!("http://localhost:{}", application_port);

    let client = reqwest::Client::builder()
        // specify redirect policy of none - otherwise it redirects back to login
        // when error and this returns a 200
        // because that's the behaviour specified in our headers
        .redirect(reqwest::redirect::Policy::none())
        // store and propogate cookies - ie send them back to the GET request from a POST request
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        port: application_port,
        test_user: TestUser::generate(),
        api_client: client,
    };

    // add a fake user_id and password to the users db
    test_app.test_user.store(&test_app.db_pool).await;

    test_app
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

/// Confirmation links embedded in the request to the email API.
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

// check where we're redirected to...
pub fn assert_is_redirect_to(response: &reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}

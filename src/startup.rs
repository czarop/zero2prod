use crate::configuration::DatabaseSettings;
use crate::configuration::Settings;
use crate::{email_client::EmailClient, routes};
use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::{dev::Server, web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use secrecy::{ExposeSecret, Secret};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

// A new type to hold the newly built server and its port
pub struct Application {
    port: u16,
    server: Server,
}
impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        // generate a connection to the database with the connection options
        // generated in configuration.rs
        // we use a pool of possible connections for concurrent queries
        let connection_pool = get_connection_pool(&configuration.database);

        // get the sender email address from config
        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender address.");

        let timeout = configuration.email_client.timeout();
        // build the client
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.auth_token,
            timeout,
        );

        // set the address an port from config file
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );

        // we want a random available port
        // specifying port 0 gives a random available port assigned by the OS
        // but we need to know which port it is so we can send requests to it
        // create a TcpListener to track which port is assigned for the server to bind
        let listener = TcpListener::bind(address)?;
        println!("Connected to {}", listener.local_addr()?);
        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            email_client,
            configuration.application.base_url,
            configuration.application.hmac_secret,
            configuration.redis_uri,
        )
        .await?;
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    // A more expressive name that makes it clear that
    // this function only returns when the application is stopped.
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    // connect lazy means no connections will be made until we need one
    PgPoolOptions::new().connect_lazy_with(configuration.connection_options())
}

// We need to define a wrapper type in order to retrieve the URL
// in the `subscribe` handler.
// Retrieval from the context, in actix-web, is type-based: using
// a raw `String` would expose us to conflicts.
pub struct ApplicationBaseUrl(pub String);

/// Starts and runs the server, as well as generating an http client pool.
///
/// # Errors
///
/// This function will return an error if the server fails to start.
pub async fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    // argument TcpListener allows us to find the port that is assigned
    // to this server by the OS - only needed if you are using a random port (port 0)

    // Wrap the pool using web::Data, which boils down to an Arc smart pointer
    let db_pool = web::Data::new(db_pool);
    // this must be done because it will be cloned onto each core, run concurrently on threads

    // same stratergy with the email client, as we want async functionality, and to
    // pass out multiple refs
    let email_client = web::Data::new(email_client);

    // this is the address we can the confirmation link to navigate to
    let base_url = web::Data::new(ApplicationBaseUrl(base_url));

    // for signed cookies, we make a location to store cookies, and register a message framework
    // this is HMAC tagginging key - defined in config base.yaml
    let signing_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(signing_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();

    // similar store but for sessions:
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    // create a server - this binds to socket and has options for
    // security etc, but needs an App to do something - passed in a closure
    let server = HttpServer::new(move || {
        // the App routes http requests coming to the server to the
        // greet handler function above
        App::new()
            // register 'middleware'
            .wrap(TracingLogger::default()) //we wrap the App in a logger - we need an implementation of the Log Trait to receive - done in main!
            .wrap(message_framework.clone()) // for secure cookies
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                signing_key.clone(),
            )) // for Sessions
            // define paths
            .route("/", web::get().to(routes::home))
            .route("/health_check", web::get().to(routes::health_check))
            .route("/login", web::get().to(routes::login_form))
            .route("/login", web::post().to(routes::login))
            .route("/admin/dashboard", web::get().to(routes::admin_dashboard))
            .route(
                "/admin/password",
                web::get().to(routes::change_password_form),
            )
            .route("/admin/password", web::post().to(routes::change_password))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .route("/subscriptions/confirm", web::get().to(routes::confirm))
            .route("/newsletters", web::post().to(routes::publish_newsletter))
            // define 'application state' - data that will be passed with the request and
            // accessible by having an argument web::Data<type> on your route receiver function
            // note you can only have one of each type of these - if need more
            // make custom types and wrap them
            .app_data(db_pool.clone()) // passes the connection to db as part of an 'application state'
            .app_data(email_client.clone()) // same for the email client
            .app_data(base_url.clone()) // same for the url for conf. email
            .app_data(web::Data::new(HmacSecret(hmac_secret.clone()))) // a secret appended to http requests so we can check it's ours
    })
    .listen(listener)? // binds to the port identified by listener
    .run(); // run the server

    //.await // Don't call await here - if you want to run other tasks async, return the server.
    // if you prefer to have the server as blocking - this fn can be async and call await here

    Ok(server)
}

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

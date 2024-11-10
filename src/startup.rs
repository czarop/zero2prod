use crate::configuration::DatabaseSettings;
use crate::configuration::Settings;
use crate::{email_client::EmailClient, routes};
use actix_web::{dev::Server, web, App, HttpServer};
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
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
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
        )?;
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
pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    base_url: String,
) -> Result<Server, std::io::Error> {
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

    // create a server - this binds to socket and has options for
    // security etc, but needs an App to do something - passed in a closure
    let server = HttpServer::new(move || {
        // the App routes http requests coming to the server to the
        // greet handler function above
        App::new()
            // route() combines a path, a set of guards, and a handler
            // here the guard is 'is it a GET request', handler is 'greet'
            // route takes a path (a string) which can have named fields - here
            // anything after the / is termed 'name', which is used in the
            // handler fn - this is called templating, but is not req.
            .wrap(TracingLogger::default()) //we wrap the App in a logger - we need an implementation of the Log Trait to receive - done in main!
            .route("/", web::get().to(routes::home))
            .route("/health_check", web::get().to(routes::health_check))
            .route("/login", web::get().to(routes::login_form))
            .route("/login", web::post().to(routes::login))
            .route("/subscriptions", web::post().to(routes::subscribe))
            .route("/subscriptions/confirm", web::get().to(routes::confirm))
            .route("/newsletters", web::post().to(routes::publish_newsletter))
            // note you can chain together commands - if the first is not met it will
            // continue to the second - both path template and guards must be satisfied
            // this is the Builder pattern
            .app_data(db_pool.clone()) // passes the connection to db as part of an 'application state'
            // this attaches extra info to the http request and is going to allow us to send updates to the db
            // you can access things attached here down the line with web::Data
            .app_data(email_client.clone()) // same for the email client
            .app_data(base_url.clone()) // same for the url for conf. email
    })
    .listen(listener)? // binds to the port identified by listener
    //.bind("127.0.0.1:8000")? // use this or listen - this binds the server to specific socket address
    .run(); // run the server

    //.await // Don't call await here - if you want to run other tasks async, return the server.
    // if you prefer to have the server as blocking - this fn can be async and call await here

    Ok(server)
}

use sqlx::postgres::PgPoolOptions;
use zero2prod::email_client;
use zero2prod::email_client::EmailClient;
use std::net::TcpListener;
use zero2prod::configuration;
use zero2prod::startup;
use zero2prod::telemetry;

#[tokio::main] // a procedural macro that wraps synchronous main() in async fn -
               // otherwise async main not allowed, and this return type not allowed
async fn main() -> Result<(), std::io::Error> {

    // set up trace and logging
    let subscriber = telemetry::get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    telemetry::init_subscriber(subscriber);

    // Panic if we can't read the config file
    let configuration =
        configuration::get_configuration().expect("Failed to read configuration.yaml");
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

    // pass any error on with ?

    // generate a connection to the database with the connection options
    // generated in configuration.rs
    // we use a pool of possible connections for concurrent queries
    let connection_pool =
        PgPoolOptions::new().connect_lazy_with(configuration.database.connection_options());
        // connect lazy means no connections will be made until we need one
    
    // get the sender email address from config
    let sender_email = configuration.email_client
        .sender()
        .expect("Invalid sender address.");

    // build the client
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.auth_token,
    );

    // await the future here - we can call main as a non-blocking
    // task in tests etc
    startup::run(listener, connection_pool, email_client)?.await?;
    Ok(())
}

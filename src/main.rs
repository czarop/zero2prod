use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup;

#[tokio::main] // a procedural macro that wraps synchronous main() in async fn -
               // otherwise async main not allowed, and this return type not allowed
async fn main() -> Result<(), std::io::Error> {
    // Panic if we can't read the config file
    let configuration = get_configuration().expect("Failed to read configuration.yaml");
    // set the address - including port from config file - this is set to 0 (random port)
    let address = format!("127.0.0.1:{}", configuration.application_port);

    // we want a random available port
    // specifying port 0 gives a random available port assigned by the OS
    // but we need to know which port it is so we can send requests to it
    // create a TcpListener to track which port is assigned for the server to bind
    let listener = TcpListener::bind(address)?;

    println!("Connected to {}", listener.local_addr()?);

    // pass any error on with ?
    // await the future here - we can call main as a non-blocking
    // task in tests etc
    startup::run(listener)?.await
}

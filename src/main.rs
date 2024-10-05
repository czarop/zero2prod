use std::net::TcpListener;

use ::zero2prod::run;

#[tokio::main] // a procedural macro that wraps synchronous main() in async fn -
               // otherwise async main not allowed, and this return type not allowed
async fn main() -> Result<(), std::io::Error> {
    // run returns a server or an error if fails to bind to socket
    //
    // we want a random available port
    // specifying port 0 gives a random available port assigned by the OS
    // but we need to know which port it is so we can send requests to it
    // create a TcpListener to track which port is assigned for the server to bind
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port");

    // pass any error on with ?
    // await the future here - we can call main as a non-blocking
    // task in tests etc
    run(listener)?.await
}

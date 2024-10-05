use std::net::TcpListener;

// checks:
// the health check is exposed at /health_check;
// the health check is behind a GET method;
// the health check always returns a 200;
// the health check’s response has no body.
#[tokio::test] // for an async test
async fn health_check_works() {
    // spawn app starts the server as async task, also returns the bound port number
    let address = spawn_app();

    // generate an http request sender
    let client = reqwest::Client::new();

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
    let server = zero2prod::run(listener).expect("Failed to launch Server");
    // launch the server as a background / non-blocking task
    let _ = tokio::spawn(server);
    // note spawn will drop all tasks when the tokio runtime is ended - so the
    // server will shut down when the test completes

    // return the bound address to the calling fn
    format!("http://127.0.0.1:{}", port)
}

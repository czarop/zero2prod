use crate::helpers::spawn_app;
use reqwest::Client;

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

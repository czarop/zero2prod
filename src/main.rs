use actix_web::{web, App, HttpRequest, HttpServer, Responder};

// a handler function for the server
// Receive an http request, and parse it for a name
async fn greet(req: HttpRequest) -> impl Responder{
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello, {}!", &name)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error>{

    // create a server - this binds to socket and has options for
    // security etc, but needs an App to do something - passed in a closure
    HttpServer::new(|| {

        // the App routes http requests coming to the server to the
        // greet handler function above
        App::new()
        // route takes a path (a string) which can have named fields - here
        // anything after the / is termed 'name', which is used in the 
        // handler fn - this is called templating
        .route("/", web::get().to(greet))
        .route("/{name}", web::get().to(greet))
        // note you can chain together commands - if the first is not met it will
        // continue to the second
    })
    .bind("127.0.0.1:8000")? // bind the server to socket address
    .run() // run the server
    .await
}

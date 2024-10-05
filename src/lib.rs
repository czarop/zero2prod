use actix_web::dev::Server;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use std::net::TcpListener;

// a handler function for the server
// Receive an http request, and parse it for a name
// return a Responder - A type implements the Responder trait if it can be
// converted into a HttpResponse
async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok() // an OK status Http response - many options in the docs
}

// we return a server instance from this - so it can be run async in main
// Note this function is not async!!

// argument TcpListener allows us to find the port that is assigned
// to this server by the OS - only needed if you are using a random port (port 0)
pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    // create a server - this binds to socket and has options for
    // security etc, but needs an App to do something - passed in a closure
    let server = HttpServer::new(|| {
        // the App routes http requests coming to the server to the
        // greet handler function above
        App::new()
            // route() combines a path, a set of guards, and a handler
            // here the guard is 'is it a GET request', handler is 'greet'
            // route takes a path (a string) which can have named fields - here
            // anything after the / is termed 'name', which is used in the
            // handler fn - this is called templating, but is not req.
            .route("/", web::get().to(greet))
            .route("/health_check", web::get().to(health_check))
            .route("/{name}", web::get().to(greet))
        // note you can chain together commands - if the first is not met it will
        // continue to the second - both path template and guards must be satisfied
        // this is the Builder pattern
    })
    .listen(listener)? // binds to the port identified by listener
    //.bind("127.0.0.1:8000")? // use this or listen - this binds the server to specific socket address
    .run(); // run the server

    //.await // Don't call await here - if you want to run other tasks async, return the server.
    // if you prefer to have the server as blocking - this fn can be async and call await here

    Ok(server)
}

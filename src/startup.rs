// Run ############################################################################

// we return a server instance from this - so it can be run async in main
// Note this function is not async!!

use crate::routes;

use actix_web::{dev::Server, web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;


// argument TcpListener allows us to find the port that is assigned
// to this server by the OS - only needed if you are using a random port (port 0)
pub fn run(listener: TcpListener, db_pool: PgPool) -> Result<Server, std::io::Error> {
    // Wrap the pool using web::Data, which boils down to an Arc smart pointer
    let db_pool = web::Data::new(db_pool);
    // this must be done because it will eb cloned onto each core, run concurrently on threads

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
            .route("/", web::get().to(routes::greet))
            .route("/health_check", web::get().to(routes::health_check))
            .route("/subscriptions", web::post().to(routes::subscribe))
            //.route("/{name}", web::get().to(greet))
            // note you can chain together commands - if the first is not met it will
            // continue to the second - both path template and guards must be satisfied
            // this is the Builder pattern
            .app_data(db_pool.clone()) // passes the connection along with the http req as part of 'application state'
                                       // this attaches extra info to the http request and is going to allow us to send updates to the db
    })
    .listen(listener)? // binds to the port identified by listener
    //.bind("127.0.0.1:8000")? // use this or listen - this binds the server to specific socket address
    .run(); // run the server

    //.await // Don't call await here - if you want to run other tasks async, return the server.
    // if you prefer to have the server as blocking - this fn can be async and call await here

    Ok(server)
}

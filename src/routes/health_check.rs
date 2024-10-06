use actix_web::{HttpRequest, HttpResponse, Responder};

// Http GET Handlers ############################################################

// a handler function for the server
// Receive an http request, and parse it for a name
// return a Responder - A type implements the Responder trait if it can be
// converted into a HttpResponse
pub async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

// handler for health check get requests
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok() // an OK status Http response - many options in the docs
}

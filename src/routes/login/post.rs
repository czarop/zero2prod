use actix_web::http::header::LOCATION;
use actix_web::{web, HttpResponse};
use secrecy::Secret;

// A struct to hold username and password
#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

pub async fn login(_form: web::Form<FormData>) -> HttpResponse {
    // this redirects back to home on submit
    HttpResponse::SeeOther()
        .insert_header((LOCATION, "/"))
        .finish()
}

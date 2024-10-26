use actix_web::{web, HttpResponse};

// defines all the query parameters that we expect to see in the incoming request
#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(_parameters))]
// If the deserialize fails from web::Query
// a 400 Bad Request is automatically returned to the caller
pub async fn confirm(_parameters: web::Query<Parameters>) -> HttpResponse {
    HttpResponse::Ok().finish()
}

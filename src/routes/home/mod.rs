use actix_web::http::header::ContentType;
use actix_web::HttpResponse;

pub async fn home() -> HttpResponse {
    // include_str! makes a static string at compile time
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("home.html"))
}

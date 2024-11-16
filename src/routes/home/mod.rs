use crate::utils::populate_dynamic_html_fields;
use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::collections::HashMap;
use std::fmt::Write;
// use crate::utils::see_other;

pub async fn home(
    flash_messages: IncomingFlashMessages, // attached if returning from failed POST req.
) -> Result<HttpResponse, actix_web::Error> {
    // check for flash message
    let mut msg_html = String::new();

    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    // Read the HTML file into a string
    let html_page = include_str!("home.html");

    // make a dict of the dynamic content
    let mut dynamic_fields = HashMap::<&str, &str>::new();
    dynamic_fields.insert("msg_html", &msg_html);

    // add the dynamic content
    let populated_html = populate_dynamic_html_fields(dynamic_fields, html_page);

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(populated_html))

    // include_str! makes a static string at compile time
    // HttpResponse::Ok()
    //     .content_type(ContentType::html())
    //     .body(include_str!("home.html"))
}

// pub async fn home_post() -> Result<HttpResponse, actix_web::Error> {
//     Ok(see_other("/login"))
// }

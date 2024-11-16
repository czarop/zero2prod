use crate::session_state::TypedSession;
use crate::utils::{e500, populate_dynamic_html_fields, see_other};
use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::collections::HashMap;
use std::fmt::Write;

pub async fn send_newsletter_form(
    session: TypedSession,                 // defined in SessionState.rs
    flash_messages: IncomingFlashMessages, // attached if returning from failed POST req.
) -> Result<HttpResponse, actix_web::Error> {
    // check for flash message
    let mut msg_html = String::new();

    // check session is valid - if not, go back to login page
    // e500 is defined in utils - just an error wrapper that preserves context
    if session.get_user_id().map_err(e500)?.is_none() {
        return Ok(see_other("/login"));
    }

    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    // Read the HTML file into a string
    let html_page = include_str!("newsletter.html");

    // make a dict of the dynamic content
    let mut dynamic_fields = HashMap::<&str, &str>::new();
    dynamic_fields.insert("msg_html", &msg_html);
    // make a random idempotency key - added as a hidden element to the page
    let key_string = String::from(uuid::Uuid::new_v4());
    dynamic_fields.insert("idempotency_key", &key_string);

    // add the dynamic content
    let populated_html = populate_dynamic_html_fields(dynamic_fields, html_page);

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(populated_html))
}

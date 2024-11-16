use actix_web::http::header::LOCATION;
use actix_web::HttpResponse;
use regex::Regex;
use std::collections::HashMap;
// use actix_web::http::StatusCode;

// take a generic, displayable error
// Return an opaque 500 while preserving the error root's cause for logging.
pub fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

pub fn e400<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static,
{
    actix_web::error::ErrorBadRequest(e)
}

// quick convenience to add location headers to a see other response
pub fn see_other(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}

// any fields that aren't specified will be populated as ""
pub fn populate_dynamic_html_fields(fields: HashMap<&str, &str>, html_string: &str) -> String {
    let re = Regex::new(r"\{(\w+)\}").unwrap();

    re.replace_all(html_string, |caps: &regex::Captures| {
        fields
            .get(&caps[1]) // caps[1] is the inner text of {text}
            .unwrap_or(&"") // if get returns None (ie the field is not in the hashmap)
    })
    .into_owned() // convert to String
}

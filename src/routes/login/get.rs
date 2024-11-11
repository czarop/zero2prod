use crate::startup::HmacSecret;
use actix_web::http::header::ContentType;
use actix_web::{web, HttpResponse};
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;

// a struct for when we get error info back from a failed login attempt
// (see post.rs)
#[derive(serde::Deserialize)]
pub struct QueryParams {
    error: String, // the error reason
    tag: String,   // the hmac tag - this is encoded as a hex string
}

impl QueryParams {
    fn verify(self, secret: &HmacSecret) -> Result<String, anyhow::Error> {
        // decode the tag
        let tag = hex::decode(self.tag)?;

        let query_string = format!("error={}", urlencoding::Encoded::new(&self.error));

        // Generate an mac verifier - first just give it the Hmac secret
        // from config base.yaml
        let mut mac =
            Hmac::<sha2::Sha256>::new_from_slice(secret.0.expose_secret().as_bytes()).unwrap();

        // add the query string to the verifier
        // a Mac (message authentication code) is made up of the
        // hmac secret tag and the query itself
        mac.update(query_string.as_bytes());

        // verify the tag we got from the http request
        mac.verify_slice(&tag)?;

        Ok(self.error)
    }
}

pub async fn login_form(
    query: Option<web::Query<QueryParams>>,
    secret: web::Data<HmacSecret>,
) -> HttpResponse {
    let error_html = match query {
        // if no error, no problem!
        None => "".into(),
        // if there's an error it could be...
        Some(query) => match query.0.verify(&secret) {
            // an legit error - maybe a wrong password or a database error
            // in which case display the error message
            Ok(error) => {
                // make sure the error message cannot be interviened and modified
                format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&error))
            }
            // an error arising from hmac verification failure - someone tampered
            // with our http requests - just go back to the login page
            Err(e) => {
                tracing::warn!(
                    error.message = %e,
                    error.cause_chain = ?e,
                    "Failed to verify query parameters using the HMAC tag"
                );
                "".into()
            }
        },
    };

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input
                type="text"
                placeholder="Enter Username"
                name="username"
            >
        </label>
        <label>Password
            <input
                type="password"
                placeholder="Enter Password"
                name="password"
            >
        </label>
        <button type="submit">Login</button>
    </form>
</body>
</html>"#,
        ))
}

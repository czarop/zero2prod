use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use crate::startup::HmacSecret;
use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::{web, HttpResponse};
use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

// A struct to hold username and password
#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    skip(pool, form, secret),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    form: web::Form<FormData>, // deserialised from httpresp
    pool: web::Data<PgPool>,
    secret: web::Data<HmacSecret>, // the secret tag we will use to verify http requests are not tampered with
                                   //specified in config, this is decoded in startup into a custom type - it just wraps a Secret<String>
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username, // form.0 as FormData wrapped in Form
        password: form.0.password,
    };

    tracing::Span::current().record("username", tracing::field::display(&credentials.username));

    // check the username and password are correct
    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));
            // if so, re-route to home
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish())
        }
        // if error, propogate it with context
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };

            let response = HttpResponse::SeeOther()
                .insert_header((LOCATION, "/login"))
                .finish();

            Err(InternalError::from_response(e, response))
        } // Err(e) => {
          //     let e = match e {
          //         AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
          //         AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
          //     };

          //     // define what went wrong in a string to be displayed to user
          //     // we encode this to prevent manipulation - this means no
          //     // new html characters can be inserted
          //     let query_string = format!("error={}", urlencoding::Encoded::new(e.to_string()));

          //     // in order to be sure no-one interferes with the http request
          //     // we tag it with an encoded secret (an hmac tag)
          //     // this will be verified...
          //     let hmac_tag = {
          //         let mut mac =
          //             Hmac::<sha2::Sha256>::new_from_slice(secret.0.expose_secret().as_bytes())
          //                 .unwrap();
          //         mac.update(query_string.as_bytes());
          //         mac.finalize().into_bytes()
          //     };

          //     let response = HttpResponse::SeeOther()
          //         .insert_header((
          //             LOCATION, // we redirect back to the login page but with a message attached
          //             format!("/login?{query_string}&tag={hmac_tag:x}"), // and the hmac tag
          //         ))
          //         .finish();
          //     // internal error allows us to add the original error (Auth Error)
          //     // and our http response
          //     Err(InternalError::from_response(e, response))
          // }
    }
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")] // this will be printed to screen if error occurs
    AuthError(#[source] anyhow::Error), // if no username or password wrong
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error), // if something fails
}

// follow the error chain back to its source for context
impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

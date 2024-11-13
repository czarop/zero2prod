use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use crate::session_state::TypedSession;
use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::Secret;
use sqlx::PgPool;

// A struct to hold username and password
#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    skip(pool, form, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    form: web::Form<FormData>, // deserialised from httpresp
    pool: web::Data<PgPool>,
    session: TypedSession, // the cookie-defined session - in our customn wrapper (see session_state)
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

            // if so, start a 'session' - ie a cookie that means the user doesn't have to
            // login again for a while.
            session.renew();

            session
                .insert_user_id(user_id) // attach the userId to the 'session' - this will be checked in admin/dashboard
                .map_err(|e| login_redirect(LoginError::UnexpectedError(e.into())))?;
            // re-route to the admin dashboard
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/admin/dashboard"))
                .finish())
        }
        // if error, propogate it with context
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };

            Err(login_redirect(e))
        }
    }
}

// failed to authenticate user
fn login_redirect(error: LoginError) -> InternalError<LoginError> {
    // the cookie is sent directly to the browser - we don't need to
    // attach it to the request
    // we set up the FlashMessage in Startup.rs
    FlashMessage::error(error.to_string()).send();

    let response = HttpResponse::SeeOther()
        // re-route to login as GET
        .insert_header((LOCATION, "/login"))
        .finish();
    InternalError::from_response(error, response)
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

use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::ExposeSecret;
use secrecy::Secret;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    // if no active session, back to login page
    if session.get_user_id().map_err(e500)?.is_none() {
        return Ok(see_other("/login"));
    };

    // check the two passwords match
    // `Secret<String>` does not implement `Eq`,
    // therefore we need to compare the underlying `String`.
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        // if they don't match - create and send a flash message - we will look for this in the GET
        // handler
        FlashMessage::error(
            "You entered two different new passwords - the field values must match.",
        )
        .send();
        // returnt hem to admin/password with a GET request
        return Ok(see_other("/admin/password"));
    }
    todo!()
}

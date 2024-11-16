use crate::authentication;
use crate::authentication::AuthError;
use crate::authentication::UserId;
use crate::routes::admin::dashboard;
use crate::utils::{e500, see_other};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,       // we need the postgres db and the session
    user_id: web::ReqData<UserId>, // this is attached in authentication::password
) -> Result<HttpResponse, actix_web::Error> {
    // if no active session, back to login page
    let user_id = user_id.into_inner();

    // we now have the user_id - not the username

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

    // check password is correct length
    if !(12..=129).contains(&form.new_password.expose_secret().len()) {
        FlashMessage::error("The new password must be between 12 & 129 characters.").send();
        return Ok(see_other("/admin/password"));
    };

    // gets the username from a user_id from postgres db
    let username = dashboard::get_username(*user_id, &pool)
        .await
        .map_err(e500)?;

    let credentials = authentication::Credentials {
        username,
        password: form.0.current_password,
    };

    // check the current password is correct
    if let Err(e) = authentication::validate_credentials(credentials, &pool).await {
        return match e {
            // wrong password - send a flash message and redirect to GET
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            }
            // smth went wrong
            AuthError::UnexpectedError(_) => Err(e500(e)),
        };
    }

    crate::authentication::change_password(*user_id, form.0.new_password, &pool)
        .await
        .map_err(e500)?;
    FlashMessage::info("Your password has been changed.").send();
    Ok(see_other("/admin/password"))
}

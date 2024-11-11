use crate::routes::subscriptions::error_chain_fmt;
use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error(transparent)]
    ConfirmSubscriberFailedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for ConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for ConfirmError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

// defines all the query parameters that we expect to see in the incoming request
#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
// If the deserialize fails from web::Query
// a 400 Bad Request is automatically returned to the caller
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmError> {
    //get the subscriber_id from the subscription token
    let id = match get_subscriber_id_from_token(&pool, &parameters.subscription_token).await {
        Ok(inner_id) => inner_id,
        Err(e) => return Err(e),
    };

    // although it's OK above, it could in theory still be none
    let id_ok = id.ok_or(anyhow::anyhow!("No user associated with the token"))?;

    match confirm_subscriber(&pool, id_ok).await {
        Ok(_) => Ok(HttpResponse::Ok().finish()),
        Err(e) => Err(e),
    }
}

/// Fetch a subsciber_id from an auth token sent in a confirmation email.
/// These are stored int he second db, subscription_tokens.
/// Returns None if no entry corresponding to that token string.
///
/// # Errors
///
/// This function will return an error if it cannot connect to the database.
#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, ConfirmError> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens \
        WHERE subscription_token = $1",
        subscription_token
    )
    .fetch_optional(pool)
    .await
    .context("No subscriber id associated with this token.")?;

    Ok(result.map(|r| r.subscriber_id))
}

/// Marks a subscriber as 'Confirmed' from 'Pending Confirmation'
/// In the database.
///
/// # Errors
///
/// This function will return an error if cannot connect to db.
#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), ConfirmError> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(pool)
    .await
    .context("Failed to confirm the subscriber in the database.")?;
    Ok(())
}

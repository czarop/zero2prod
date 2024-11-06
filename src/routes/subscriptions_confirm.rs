use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use crate::routes::subscriptions::error_chain_fmt;

#[allow(dead_code)]
pub struct RetrieveTokenError(sqlx::Error);

impl std::fmt::Display for RetrieveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to retrieve a subscription token."
        )
    }
}

impl std::fmt::Debug for RetrieveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for RetrieveTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    //<-  the message that will be displayed. '0' means the first parameter of the declaration below, here 'String'
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error), // <- we have 1 parameter , an anyhow::Error, and our error type can be generated from this
} // also check out #[source]

// anyhow::error converts the error returned by our methods into an anyhow::Error;
// it enriches it with additional context around the intentions of the caller.
// it can get context from any Result - via an extension trait - which is all taken care of.

// define what is printed in debug log
impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // go as far back in the error chain as you can to ID the source
        error_chain_fmt(self, f)
    }
}
// Response error is from Actix:web and we impl it so that we can use it with
// that crate
// note it has default implementation to return 500 internal server error
// we override to but give different responses from each error type
impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            // bad request when email address can't be validated
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            // otherwise internal server error
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
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
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
    //get the subscriber_id from the subscription token
    let id = match get_subscriber_id_from_token(&pool, &parameters.subscription_token).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    match id {
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            if confirm_subscriber(&pool, subscriber_id).await.is_err() {
                return HttpResponse::InternalServerError().finish();
            } else {
                HttpResponse::Ok().finish()
            }
        }
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
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens \
        WHERE subscription_token = $1",
        subscription_token
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(result.map(|r| r.subscriber_id))
}

/// Marks a subscriber as 'Confirmed' from 'Pending Confirmation'
/// In the database.
///
/// # Errors
///
/// This function will return an error if cannot connect to db.
#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {:?}", e);
        e
    })?;
    Ok(())
}

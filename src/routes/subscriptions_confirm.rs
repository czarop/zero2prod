use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;

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
#[tracing::instrument(
    name = "Get subscriber_id from token",
    skip(subscription_token, pool)
)]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str
) -> Result<Option<Uuid>, sqlx::Error>{
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens \
        WHERE subscription_token = $1",
        subscription_token
    )
    .fetch_optional(pool)
    .await.map_err(|e| {
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
#[tracing::instrument(
    name = "Mark subscriber as confirmed",
    skip(subscriber_id, pool)
)]
pub async fn confirm_subscriber(
    pool: &PgPool,
    subscriber_id: Uuid
) -> Result<(), sqlx::Error> {
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

use crate::idempotency;
use crate::{
    authentication::UserId,
    domain::SubscriberEmail,
    email_client::EmailClient,
    idempotency::IdempotencyKey,
    utils::{e400, e500, see_other},
};
use actix_web::web::ReqData;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, pool, email_client, user_id),
    fields(user_id=%*user_id)
)]
pub async fn send_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,  // we need the postgres db and the session
    user_id: ReqData<UserId>, // extracted from the user session
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    // We must destructure the form to avoid upsetting the borrow-checker
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;

    // get the key & convert to our strongly typed version
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    // see if we already have a corresponding entry in the idempotency db
    let transaction = match idempotency::try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        // if we don't, we receive an sqlx transaction - started in idempotency::try_processing() -
        // see further explanation in that fn
        idempotency::NextAction::StartProcessing(transaction) => transaction,
        // return early if we have a saved response in the idempotency db
        idempotency::NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            // return the saved response - don't create a new one
            return Ok(saved_response);
        }
    };

    // get the subscribers
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;

    // iterate
    for subscriber in subscribers {
        // check it's legit
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email, &title, &text_content, &html_content)
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })
                    .map_err(e500)?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    error.message = %error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }
    success_message().send();
    let response = see_other("/admin/newsletter");

    // insert this request into the idempotency database
    let response = idempotency::save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
    // We are returning a `Vec` of `Result`s in the happy case.
    // This allows the caller to bubble up errors due to network issues or other
    // transient failures using the `?` operator, while the compiler
    // forces them to handle the subtler mapping error.
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    // we'll collect from sqlx into a basic String
    struct Row {
        email: String,
    }

    // query_as! maps the retrieved rows to the type specified as its first argument
    let confirmed_subscribers = sqlx::query_as!(
        Row,
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    // No longer using `filter_map`!
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();
    Ok(confirmed_subscribers)
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

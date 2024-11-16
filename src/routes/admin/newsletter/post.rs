use crate::{
    authentication::UserId,
    domain::SubscriberEmail,
    email_client::EmailClient,
    utils::{e500, see_other},
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
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, pool, email_client, user_id),
    fields(user_id=%*user_id)
)]
pub async fn send_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>, // we need the postgres db and the session
    user_id: ReqData<UserId>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    // get the subscribers
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;

    // iterate
    for subscriber in subscribers {
        // check it's legit
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &form.0.title,
                        &form.0.text_content,
                        &form.0.html_content,
                    )
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
    FlashMessage::info("The newsletter issue has been published!").send();
    Ok(see_other("/admin/newsletter"))
}

/// Converts plain text to HTML-safe text
// fn convert_to_html(input: &str) -> String {

//     input
//         .replace("&", "&amp;")  // Escape `&`
//         .replace("<", "&lt;")   // Escape `<`
//         .replace(">", "&gt;")   // Escape `>`
//         .replace("\"", "&quot;") // Escape `"`
//         .replace("'", "&#039;")  // Escape `'`
//         .replace("\n", "<br>")   // Convert newlines to `<br>`
// }

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

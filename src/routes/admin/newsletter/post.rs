use crate::idempotency;
use crate::{
    authentication::UserId,
    idempotency::IdempotencyKey,
    utils::{e400, e500, see_other},
};
use actix_web::web::ReqData;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip_all,
    fields(user_id=%&*user_id)
)]
pub async fn send_newsletter(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,  // we need the postgres db and the session
    user_id: ReqData<UserId>, // extracted from the user session
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
    let mut transaction = match idempotency::try_processing(&pool, &idempotency_key, *user_id)
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

    // insert the newsletter into our 'newsletter issue status' table,
    // getting a unique ID for it. This table records pending tasks
    // and whether they have been completed or not
    let newsletter_issue_id =
        insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
            .await
            .context("Failed to store newsletter issue details")
            .map_err(e500)?;

    // make the list of email addresses to send the nesletter to
    // in another table
    // adding everything to the same sqlx transaction
    // so it can be executed in one go, and rolled back if required
    enqueue_delivery_tasks(&mut transaction, newsletter_issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;

    let response = see_other("/admin/newsletter");

    // insert this request into the idempotency database
    let response = idempotency::save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;

    success_message().send();
    Ok(response)
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been queued for publishing!")
}

// A newsletter delivery task - with status (has it been sent to everytone or not)
#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    // unique id for this newsletter issue
    let newsletter_issue_id = Uuid::new_v4();

    // insert the newsetter into the newsletter table
    let query = sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id,
            title,
            text_content,
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    );

    // execute the transaction
    transaction.execute(query).await?;
    Ok(newsletter_issue_id)
}

// a queue of email addresses to send the newsletter to
#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
    );
    transaction.execute(query).await?;
    Ok(())
}

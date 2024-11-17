use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::{configuration::Settings, startup};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::time::Duration;
use tracing::{field::display, Span};
use uuid::Uuid;
// used to define if there is a task in the queue or not
pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(
    skip_all,
    fields(
    newsletter_issue_id=tracing::field::Empty,
    subscriber_email=tracing::field::Empty
    ),
    err
)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    // send the emails
    let task = dequeue_task(pool).await?;
    // if there are no tasks in the emais to send table... bail out
    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }

    // otherwise, proceed
    let (transaction, issue_id, email) = task.unwrap();

    Span::current()
        .record("newsletter_issue_id", display(issue_id))
        .record("subscriber_email", display(&email));

    // remove the task from the queue - this commits the transaction
    delete_task(transaction, issue_id, &email).await?;

    // NOTE - we do not retry to send - if the below fails, it has already
    // been removed from the queue. You can implement this easily enough -
    // keep track of number of retries for that row (add another column) and
    // keep the row in the queue until it is successful or has had x retries

    // try to parse the email address into our Subscriber Email type
    match SubscriberEmail::parse(email.clone()) {
        Ok(email_address) => {
            // get the email body to send
            let issue = get_issue(pool, issue_id).await?;
            // try to send the email
            if let Err(e) = email_client
                .send_email(
                    &email_address,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await
            {
                // if error sending the email, log it
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Failed to deliver issue to a confirmed subscriber. Skipping.",
                );
            }
        } // if an error parsing the email address, log it
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "Skipping a confirmed subscriber. Their stored contact details are invalid",
            );
        }
    }

    Ok(ExecutionOutcome::TaskCompleted)
}

// make a short name for the sqlx transaction
type PgTransaction = Transaction<'static, Postgres>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    // get the first row of the 'email's to send' queue - actually
    // the first one that is not locked by another thread - we will have
    // multiple threads sending these out
    let row = sqlx::query!(
        r#"
            SELECT newsletter_issue_id, subscriber_email
            FROM issue_delivery_queue
            FOR UPDATE
            SKIP LOCKED
            LIMIT 1
        "#,
    )
    .fetch_optional(&mut *transaction) // get a row
    .await?;

    // check we have a row
    if let Some(row) = row {
        // if so, return the transaction with row details
        Ok(Some((
            transaction,
            row.newsletter_issue_id,
            row.subscriber_email,
        )))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    // remove the row from the delivery queue table and execute the transaction
    let query = sqlx::query!(
        r#"
            DELETE FROM issue_delivery_queue
            WHERE
                newsletter_issue_id = $1 AND
                subscriber_email = $2
        "#,
        issue_id,
        email
    );
    transaction.execute(query).await?;
    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

// we need to get our hands on the newsletter issue to send
// this is in our 'Newsletters to Send' table
#[tracing::instrument(skip_all)]
async fn get_issue(pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
            SELECT title, text_content, html_content
            FROM newsletter_issues
            WHERE
                newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}

// an infinite loop that attempts to complete all tasks
async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        // if there is nothing in the db but task is not completed,
        // wait a few seconds and retry
        // if there's an error wait 1 second and retry
        // when task completed, return
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

// use the above fn to complete all tasks - this is run as a task in Main()
pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    // get a separate connection tot he db - note we don't NEED to do this
    // could get an ARC pointer as we have been doing elsewhere
    let connection_pool = startup::get_connection_pool(&configuration.database);

    // get the client from config
    let email_client = configuration.email_client.client();

    // start sending
    worker_loop(connection_pool, email_client).await
}

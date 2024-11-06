use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

// Http POST Handler ############################################################

// define a new error type for errors when storing tokens - we just want
// to wrap sqlx::Error in a new type so that we can implement a trait on it from
// actix::web - but we need to also impl debug and display...

#[allow(dead_code)]
pub struct StoreTokenError(sqlx::Error);

// we write explicit executions of display and debug - rather than derived ones
impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // defined below - backtracks through error chain to find the true source
        error_chain_fmt(self, f)
    }
}

// we can also impl source - this is a trait of std:error::Error
// and gives us access to the source of the error, fromt he receiver of the error
impl std::error::Error for StoreTokenError {
    // dyn is dynamically sized - very similar to generics but will be sized at runtime
    // you use dyn when you don't know the concrete types at compile time
    // as returning a reference, it requires a lifetime, which here is static
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0) // &sqlx::Error will be cast to &dyn Error
    }
}

// general error fn that iterates over the whole chain of errors
// backtracking through the source() fn on Error trait
pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

// thiserror macro makes all the error types and controls their source and from implementations
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

// handler for subscribe post requests - the fn is going to extract form data from a
// post request. It needs a struct containing the form datafields as such:
#[derive(serde::Deserialize)] // this automatically implements deserialise for the specified struct!
                              // which allows the http req to be parsed into the struct
pub struct FormData {
    email: String, // these fields must be specified in the http req
    name: String,
}

// and the handler itself - it must accept a web::Form<FormData> - ie the struct above
// All arguments in the signature of a route handler must implement the
// FromRequest trait, which means the info can be extracted, or deserialised - you can then
// work with the extracted data instead of parsing an HttpReq
// basically it's all taken care of using the struct above and the serde:deserialise macro!

// implement try_from Form Data for new subscriber struct
impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email, name })
    }
}

/// Accepts username and email as a web form, performs validity checking and if passes
/// inserts a new entry into the database.

#[tracing::instrument( // this macro registers everything that happens in the below fn as part of a new SPAN
    name = "Adding a new subscriber", //a message associated to the function span
    // all fn args are automatically added to the log
    skip(form, connection_pool, email_client, base_url), // we don't want to log stuff about these variables
    fields( // here we can add futher things of explicitly state how you want to display things
    subscriber_email = %form.email,
    subscriber_name = %form.name // the % - we are telling tracing to use their Display implementation
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>, // FormData defined above
    connection_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>, //form data contains
    // our http request info in FormData but also anything attached with .app_data(data) in Web::Data <- we did this
    // with email_client and PgPool in the Run fn in Startup.rs
    base_url: web::Data<ApplicationBaseUrl>, // address for the confirmation email
) -> Result<HttpResponse, SubscribeError> {
    // web::form is a wrapper around FormData (Form<FormData>) -
    // access the formdata by form.0
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;

    // create an sqlx 'transaction' that groups together sqlx queries so that you don't
    // get stuck in an interim state if the program crashes 1/2 way through
    // call queries on this instead of pool
    let mut transaction = connection_pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    // whatever the error - we get a box pointer to it and wrap it in UnexpectedError
    // Box pointer as we own the data (so can't be a reference) and UnexpectedError accepts
    // a dynamic type (dyn) which cannot be sized at compile time

    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database.")?;

    let subscription_token = generate_subscription_token();

    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber.")?;

    // commit the transaction - ie make changes to the db permanent
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber.")?;

    send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Store subscription token in the database"
    skip(subscription_token, transaction)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );
    transaction.execute(query).await.map_err(StoreTokenError)?;
    Ok(())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    // make a confirmation link - inlcude a subscription token
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );

    let html_body = &format!(
        "Welcome to our newsletter!<br />\
           Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );

    let plain_text_body = &format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );

    // send a confirmation email to the new subscriber
    email_client
        .send_email(
            new_subscriber.email,
            "Welcome!!",
            html_body,
            plain_text_body,
        )
        .await
}

#[tracing::instrument(
    name = "saving new subscriber in the database",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();

    // insert form data to the db with this query
    let query = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(), // the &str of our username type inner value
        Utc::now()                    // timestamp
    );

    transaction.execute(query).await?; // Using the `?` operator to return early

    Ok(subscriber_id)
}

// a random sequence of alphanumeric chars
fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

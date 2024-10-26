use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

// Http POST Handler ############################################################

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
) -> HttpResponse {
    // web::form is a wrapper around FormData (Form<FormData>) -
    // access the formdata by form.0
    let new_subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber, // NewSubscriber defined in domain::new_subscriber
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    // try inserting the subscriber to db
    if insert_subscriber(&connection_pool, &new_subscriber)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    let send_email_failed = send_confirmation_email(&email_client, new_subscriber, &base_url.0)
        .await
        .is_err();

    if send_email_failed {
        return HttpResponse::InternalServerError().finish();
    } else {
        return HttpResponse::Ok().finish();
    }
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
) -> Result<(), reqwest::Error> {
    // make a confirmation link - inlcude a subscription token
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token=mytoken",
        base_url
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
    skip(new_subscriber, connection_pool)
)]
pub async fn insert_subscriber(
    connection_pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    // insert form data to the db with this query
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(), // the &str of our username type inner value
        Utc::now()                    // timestamp
    )
    // We use `get_ref` to get an immutable reference to the `PgConnection`
    // wrapped by `web::Data`.
    .execute(connection_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?; // Using the `?` operator to return early
         // if the function failed, returning a sqlx::Error
    Ok(())
}

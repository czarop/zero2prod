use crate::domain::SubscriberEmail;
use crate::{email_client::EmailClient, routes::error_chain_fmt};
use actix_web::http::{
    header::{HeaderMap, HeaderValue},
    StatusCode,
};
use actix_web::{web, HttpRequest, HttpResponse, ResponseError};
use anyhow::Context;
use base64::Engine;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

// a couple of structs to deserialise a newsletter email -
// These will convert an incoming html message to the API
// to a newsletter structure....
#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
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


// create a new 'span' around this fn, so we can add the user_id
// to logs
#[tracing::instrument(
    name = "Publish a newsletter",
    skip(body, pool, email_client, request),
    fields(
        username=tracing::field::Empty, // these will be filled in during the fn
        user_id=tracing::field::Empty,
    ),
)]
// gets a list of confirmed subscriber email addresses
// the body and pool will be passed in the application context from main
pub async fn publish_newsletter(
    body: web::Json<BodyData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest, // the request triggering the call
) -> Result<HttpResponse, PublishError> {
    // check credentials in request headers are ok before proceeding
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;
    // record in tracing log (see above)
    tracing::Span::current().record(
        "username",
        tracing::field::display(&credentials.username)
    );
    // get the user id for this username fromt he sqlx table
    let user_id = validate_credentials(credentials, &pool).await?;
    // record in log
    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));

    // get our list of confirmed subscribers
    let subscribers = get_confirmed_subscribers(&pool).await?;

    // fire the emails... one by one
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(
                        &subscriber.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    .with_context(|| {
                        // in the case of an error, this closure will be run to add context to the error
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                // We record the error chain as a structured field
                // on the log record.
                error.cause_chain = ?error,
                "Skipping a confirmed subscriber. \
                Their stored contact details are invalid",
                );
            }
        }
    }
    Ok(HttpResponse::Ok().finish())
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication Failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)] // a transparent error gets its message from context
    UnexpectedError(#[from] anyhow::Error),
}
// Same logic to get the full error chain on `Debug`
impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}
impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                // create headers for the error response
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                // add the headers
                response
                    .headers_mut()
                    .insert(actix_web::http::header::WWW_AUTHENTICATE, header_value);

                response
            }
        }
    }
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // The header value, if present, must be a valid UTF8 string
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    // Split into two segments, using ':' as delimiter
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();

    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    // get user_id from user table if it matches user and password
    // passed out as a record (ie a row)
    // we use fetch_optional - which gives a single (first) row that matches
    let user_id = sqlx::query!(
        r#"
        SELECT user_id
        FROM users
        WHERE username = $1 AND password = $2
        "#,
        credentials.username,
        credentials.password.expose_secret()
    )
    
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")
    .map_err(PublishError::UnexpectedError)?;

    // get the user id from the record - or error if none
    user_id
        .map(|row| row.user_id)
        // if we can't find - we create an anyhow::Error with attached context
        // because we can't make a new PublishError directly - we need to pass it 
        // another error type, or convert into it
        .ok_or_else(|| anyhow::anyhow!("Invalid username or password."))
        .map_err(PublishError::AuthError) // switch the error type explicitly
}

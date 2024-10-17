use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

// Http POST Handler ############################################################

// handler for subscribe post requests - the fn is going to extract form data from a
// post request. It needs a struct containing the form datafields as such:
#[derive(serde::Deserialize)] // this automatically implements deserialise for the specified struct!
                              // which allows the http req to be parsed into the struct
pub struct FormData {
    email: String,
    name: String,
}
// and the handler itself - it must accept a web::Form<FormData> - ie the struct above
// All arguments in the signature of a route handler must implement the
// FromRequest trait, which means the info can be extracted, or deserialised - you can then
// work with the extracted data instead of parsing an HttpReq
// basically it's all taken care of using the struct above and the serde:deserialise macro!

// this macro registers everything that happens in the below fn as part of a new SPAN
#[tracing::instrument(
    name = "Adding a new subscriber", //a message associated to the function span
    // all fn args are automatically added to the log - but we may not want all
    skip(form, connection_pool), // we don't want to log stuff about these variables
    fields( // here we can add futher things of explicitly state how you want to display things
    request_id = %Uuid::new_v4(), // you can use the variable name as a key
    subscriber_email = %form.email, // or explicitly name them
    subscriber_name = %form.name // the % - we are telling tracing to use their Display 
                                // implementation for logging purposes
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    connection_pool: web::Data<PgPool>,
) -> HttpResponse {
    match insert_subscriber(&connection_pool, &form).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(
    name = "saving new subscriber in the database",
    skip(form, connection_pool)
)]
pub async fn insert_subscriber(
    connection_pool: &PgPool,
    form: &FormData,
) -> Result<(), sqlx::Error> {
    // insert form data to the db with this query
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now() // timestamp
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

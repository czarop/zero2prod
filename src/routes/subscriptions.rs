use actix_web::{web, HttpResponse, Responder};
use sqlx::PgPool;
use chrono::Utc;
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

// connection is an extractor - it extracts from the app_data() call in startup() - and finds an
// instance of the specified type (which we passed)
pub async fn subscribe(form: web::Form<FormData>, connection_pool: web::Data<PgPool>) -> impl Responder {
    // insert form data to the db with this query
    let query_result = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(), // random id
        form.email,
        form.name,
        Utc::now() // timestamp
        )
        // We use `get_ref` to get an immutable reference to the `PgConnection`
        // wrapped by `web::Data`.
        .execute(connection_pool.get_ref())
        .await;

    match query_result{
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("Failed to execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
    

}

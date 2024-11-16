use super::IdempotencyKey;
use actix_web::body::to_bytes;
use actix_web::{http::StatusCode, HttpResponse};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// fetch a saved HTTP response from the store - ie any response
/// matching this user_id and idempotency key
pub async fn get_saved_response(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    // interrogate the db:

    // to use custom types (such as our header_pair type)
    // it must impl a few traits - which can be derived
    // this is done below.. note this struct has the same
    // layout as the one we defined int he sqlx migration
    // when generating this db table!
    // note response headers below - you need to specifically tell
    // the macro how to deal with this custom type
    // the as "name!" parts are to tell sqlx these columns will not be null
    // as we have relaxed these rules with one of the migrations
    let saved_response = sqlx::query!(
        r#"
            SELECT
            response_status_code as "response_status_code!",
            response_headers as "response_headers!: Vec<HeaderPairRecord>",
            response_body as "response_body!"
            FROM idempotency
            WHERE
            user_id = $1 AND
            idempotency_key = $2
            "#,
        user_id,
        idempotency_key.as_ref()
    )
    .fetch_optional(pool)
    .await?;

    // if there's a row... unwrap it
    if let Some(r) = saved_response {
        // get the status code
        let status_code = StatusCode::from_u16(r.response_status_code.try_into()?)?;
        // build the response from the response headers we find
        let mut response = HttpResponse::build(status_code);
        // iterate through the headers and append them to response
        for HeaderPairRecord { name, value } in r.response_headers {
            response.append_header((name, value));
        }
        // r.response_body is the email text
        Ok(Some(response.body(r.response_body)))
    } else {
        Ok(None)
    }
}

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")] // tells sqlx the 'sqlx' name of this type
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

// this was in the book but doesn't appear to be needed..... maybe derived now

// because we make a vec of header_pairs, we also need to tell sqlx the name
// to append to refer to it by when it's in a vec (or any 'composite type')
// impl PgHasArrayType for HeaderPairRecord {
//     fn array_type_info() -> sqlx::postgres::PgTypeInfo {
//         sqlx::postgres::PgTypeInfo::with_name("_header_pair")
//     }
// }

/// save an httpResponse to the database with an idempotency key
/// working with httpresponse is tough - to access the body we need to:
/// Get ownership of the body via .into_parts();
/// Buffer the whole body in memory via to_bytes;
/// inset the info (incl body) into db;
/// Re-assemble the response using .set_body() on the request head
/// return the response
pub async fn save_response(
    mut transaction: Transaction<'static, Postgres>, // an sqlx transaction - ie 1 or more queries executed together
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
    http_response: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    // get ownership of the body - note the type is boxbody - a generic type
    // from which http responses are derived
    // basically either a bytes type (data transferred in one go) or a stream type
    let (response_head, body) = http_response.into_parts();

    // Buffer the whole body in memory via to_bytes;
    // note for larger http requests - ie with file attachments - to_bytes()
    // loads everything to server memory in one go, instead you'd want to send
    // it as a stream
    let body = to_bytes(body)
        .await
        // `MessageBody::Error` is not `Send` + `Sync`,
        // therefore it doesn't play nicely with `anyhow`
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // get the status code
    let status_code = response_head.status().as_u16() as i16;

    // get the headers (loaded into out HeaderPairRecord type)
    let headers = {
        // make a vec to store the headers
        let mut h = Vec::with_capacity(response_head.headers().len());
        for (name, value) in response_head.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();
            h.push(HeaderPairRecord { name, value });
        }
        h
    };

    // insert the entry into the postgres db
    // we have to use an unchecked query as the macro doesn't recognise
    // our custom type HeaderPairRecord :-(
    let query = sqlx::query_unchecked!(
        r#"
        UPDATE idempotency
        SET
            response_status_code = $3,
            response_headers = $4,
            response_body = $5
        WHERE
            user_id = $1 AND
            idempotency_key = $2
        "#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        headers,
        body.as_ref(),
    );

    // attach another query to the sqlx transaction
    transaction.execute(query).await?;

    // finally we actually commit the transaction
    transaction.commit().await?;

    // Re-assemble the response using .set_body() on the request head.
    let http_response = response_head.set_body(body).map_into_boxed_body(); // back to the generic type behind httpresponses

    // return the http response - what a faff!
    Ok(http_response)
}

// an enum to group potential results of trying to insert a new row into
// idempotency db
#[allow(clippy::large_enum_variant)]
pub enum NextAction {
    StartProcessing(Transaction<'static, Postgres>), // a sqlx transaction - see below
    ReturnSavedResponse(HttpResponse),
}

/// see if there is already a matching entry in the idempotency db
/// we will do this by trying to insert a new row, and seeing if
/// a row actually gets inserted or there is a CONFLICT
pub async fn try_processing(
    pool: &PgPool,
    idempotency_key: &IdempotencyKey,
    user_id: Uuid,
) -> Result<NextAction, anyhow::Error> {
    // we will perform both this INSERT query and any concurrent
    // UPDATE queries (in saved_response()) as a single transaction - this means
    // the concurrent INSERT will wait for the UPDATE to complete, instead
    // of causing an error. After the UPDATE completes, there will be a
    // matching row and the 2nd INSERT will cause a CONFLICT, and return
    // the existing row's httpResponse
    let mut transaction = pool.begin().await?;

    // try to insert the data, and
    // calculate the number of rows inserted by the call
    let query = sqlx::query!(
        r#"
        INSERT INTO idempotency (
            user_id,
            idempotency_key,
            created_at
        )
        VALUES ($1, $2, now())
        ON CONFLICT DO NOTHING  
        "#,
        user_id,
        idempotency_key.as_ref()
    );

    let n_inserted_rows = transaction.execute(query).await?.rows_affected(); // how many rows inserted

    if n_inserted_rows > 0 {
        // if >0 rows inserted, start sending out emails
        Ok(NextAction::StartProcessing(transaction)) // attach the transaction
    } else {
        // if not, get the row it clashed with - this is your saved httpresponse
        let saved_response = get_saved_response(pool, idempotency_key, user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("We expected a saved response, we didn't find it"))?;
        // else pass back the enum with the old http request
        Ok(NextAction::ReturnSavedResponse(saved_response))
    }
}

use crate::utils::e500;
use actix_web::http::header::LOCATION;
use actix_web::{http::header::ContentType, web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::session_state::TypedSession;

pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // we stored the user_id into the session state as part of login -
    // now we retrieve that with session::get("user_id")
    // this gives us the username to look up in the redis db and check
    // their cookie session state is ok

    // this reads, if session.get("user_id") returns Some(user_id), {username = x} else {username = y}

    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &pool).await.map_err(e500)?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta http-equiv="content-type" content="text/html; charset=utf-8">
            <title>Admin dashboard</title>
        </head>
        <body>
            <p>Welcome {username}!</p>
            <p>Available actions:</p>
            <ol>
                <li><a href="/admin/password">Change password</a></li>
                <li><a href="/admin/newsletter">Send a newsletter</a></li>
                <li>
                    <form name="logoutForm" action="/admin/logout" method="post">
                    <input type="submit" value="Logout">
                    </form>
                </li>
            </ol>
        </body>
        </html>"#
        )))
}

// Interrogate the redis db for a Session associated with user_id x
#[tracing::instrument(name = "Get Username", skip(pool))]
pub async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE user_id = $1
        "#,
        user_id,
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username.")?;

    Ok(row.username)
}

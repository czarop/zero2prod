use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

// this is called when you navigate to /login but also
// you are redirected here after POSTing login credentials
// - if the latte, there will be a cookie attached with
// error info
pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    // empty String to load an error into
    let mut error_html = String::new();

    // A message will be there if we are redirected
    // from a failed login POST request - there may be multiple messages of course!
    // look for Error level messages only
    for message in flash_messages.iter()
    // .filter(|message| message.level() == Level::Error)
    {
        writeln!(error_html, "<p><i>{}</i></p>", message.content()).unwrap();
    }

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Login</title>
</head>
<body>
    {error_html}
    <form action="/login" method="post">
        <label>Username
            <input
                type="text"
                placeholder="Enter Username"
                name="username"
            >
        </label>
        <label>Password
            <input
                type="password"
                placeholder="Enter Password"
                name="password"
            >
        </label>
        <button type="submit">Login</button>
    </form>
</body>
</html>"#,
        ))
}

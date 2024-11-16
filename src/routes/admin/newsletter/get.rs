use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};
use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use std::fmt::Write;

pub async fn send_newsletter_form(
    session: TypedSession,                 // defined in SessionState.rs
    flash_messages: IncomingFlashMessages, // attached if returning from failed POST req.
) -> Result<HttpResponse, actix_web::Error> {
    // check for flash message
    let mut msg_html = String::new();

    // check session is valid - if not, go back to login page
    // e500 is defined in utils - just an error wrapper that preserves context
    if session.get_user_id().map_err(e500)?.is_none() {
        return Ok(see_other("/login"));
    }

    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Send a Newsletter</title>
    <style>
    /* Reset default styling */
    body {{
      margin: 0;
      padding: 10px; /* Add some padding around the body for aesthetics */
      box-sizing: border-box;
        }}

    input, textarea {{
      width: 100%; /* Full width of the container */
      max-width: 100%; /* Prevent overflow */
      box-sizing: border-box; /* Include padding in width calculation */
      margin: 0; /* Reset default margins */
      padding: 8px; /* Add some padding for usability */
      font-size: 16px; /* Ensure text consistency */
        }}

    textarea {{
      resize: none; /* Optional: Disable resizing for consistent design */
        }}
  </style>
</head>
<body>
    {msg_html}
    <form action="/admin/newsletter" method="post">
        <h3>Newsletter Title:</h3>
        <input
            type="text"
            style="width:100%;font-family:Courier"
            placeholder="Enter a title"
            name="title"
        >
    <br><br>
    <h3>Email Content as Plain Text:</h3>
    <textarea
        size="200"
        style="width:100%;height:500px;resize: none"
        placeholder="Enter content"
        name="text_content"
    ></textarea>
    </label>
    <br><br>
    <h3>Email Content as HTML:</h3>
    <textarea
        size="200"
        style="width:100%;height:500px;resize: none"
        placeholder="Enter content"
        name="html_content"
    ></textarea>
    </label>
        <br><br>
        <button type="submit">Send Newsletter</button>
    </form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#,
        )))
}

use actix_web::{web, HttpResponse, Responder};

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
pub async fn subscribe(_form: web::Form<FormData>) -> impl Responder {
    HttpResponse::Ok() // this will actually return a 400 error if the form data
                       // cannot be parsed into the struct
}

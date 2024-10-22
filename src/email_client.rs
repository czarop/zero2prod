use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{Secret, ExposeSecret};

#[derive(serde::Serialize)]
struct SendEmailRequest {
    from: String,
    to: String,
    subject: String,
    html_body: String,
    text_body: String,
}



// these are costly to connect - instead we make one instance and get refs to it
// whenever sending an email.
// this is created in startup.rs run()

pub struct EmailClient {
    http_client: Client,
    base_url: reqwest::Url,
    sender: SubscriberEmail,
    auth_token: Secret<String>,
}

impl EmailClient {

    pub fn new(base_url: String, sender: SubscriberEmail, auth_token: Secret<String>) -> Self {
        Self { http_client: Client::new(), 
            base_url: reqwest::Url::parse(&base_url).expect(format!("Could not parse url {}", base_url).as_str()), 
            sender: sender ,
            auth_token: auth_token}
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        
        // Need to build a request that looks like this:
        // curl "https://api.postmarkapp.com/email" \
        //     -X POST \
        //     -H "Accept: application/json" \
        //     -H "Content-Type: application/json" \
        //     -H "X-Postmark-Server-Token: server token" \
        //     -d '{
        //     "From": "sender@example.com",
        //     "To": "receiver@example.com",
        //     "Subject": "Postmark test",
        //     "TextBody": "Hello dear Postmark user.",
        //     "HtmlBody": "<html><body><strong>Hello</strong> dear Postmark user.</body></html>"
        //     }'


        let url = self.base_url.join("/email").expect(format!("Failed to parse base URL {}", self.base_url).as_str());
        
        let request_body = SendEmailRequest {
            from: self.sender.as_ref().to_owned(),
            to: recipient.as_ref().to_owned(),
            subject: subject.to_owned(),
            html_body: html_content.to_owned(),
            text_body: text_content.to_owned(),
        };

        let builder = self
            .http_client
            .post(url)
            .header(
                "X-Postmark-Server-Token", 
                self.auth_token.expose_secret())
            .json(&request_body)
            .send()
            .await?;

        Ok(())

    }
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers;
    use wiremock;


    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {

        // Arrange
        let mock_server = wiremock::MockServer::start().await; // this is a real server run on a thread!
        // make a fake sender email address and wrap it in our wrapper
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        // make an email client
        let address = mock_server.uri(); // the address the server is running on 
        let email_client = EmailClient::new(address, sender, Secret::new(Faker.fake()));
        
        // give the mock server some parameters by 'mounting' a Mock
        // when the server receives a request it iterates over all Mocks
        // to check if the request matches thier conditions
        wiremock::Mock::given(matchers::any()) // given specifies the conditions - any() is 
            .respond_with(wiremock::ResponseTemplate::new(200)) // normally responds with 404 to everything
            .expect(1) // server should expect 1 request only - this is verfied when the test ends
            .mount(&mock_server) // mounts only work if 'mounted' on the mock server
            .await;

        // make a fake email address and some content
        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();
        
        // Act
        let _ = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
        
        // Assert
    }

}
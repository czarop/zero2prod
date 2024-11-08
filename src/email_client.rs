use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")] // ensures pascal case for html
struct SendEmailRequest<'a> {
    from: &'a str, // these refs live as long as the struct
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

// these are costly to connect - instead we make one instance and get refs to it
// whenever sending an email.
// this is created in startup.rs run()

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    auth_token: Secret<String>,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        auth_token: Secret<String>,
        timeout: std::time::Duration,
    ) -> Self {
        // create a client with a timeout of 10s if no response from server
        let http_client = Client::builder().timeout(timeout).build();

        let http_client = match http_client {
            Ok(client) => client,
            Err(_) => panic!("Cannot create server"),
        };
        // let url = reqwest::Url::parse(&base_url)
        // .expect(format!("Could not parse url {}", base_url).as_str());

        // create the email client wrapper
        Self {
            http_client,
            base_url,
            sender,
            auth_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: &SubscriberEmail,
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

        // this is firing to https://api.postmarkapp.com/email
        let url = format!("{}/email", self.base_url);

        println!(
            "{}\n {}\n {}",
            url.as_str(),
            self.sender.as_ref(),
            self.auth_token.expose_secret().as_str()
        );
        println!("{}\n {}", recipient.as_ref(), html_content);

        let request_body = SendEmailRequest {
            from: self.sender.as_ref(), // we could put these as 'to_owned' and have them as Strings
            to: recipient.as_ref(),
            subject,
            html_body: html_content,
            text_body: text_content,
        };

        self.http_client
            .post(&url)
            // .header("Accept", "application/json")
            // .header("Content-Type", "application/json")
            .header("X-Postmark-Server-Token", self.auth_token.expose_secret())
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?; // converts an error code, e.g. 404, into a reqwest error

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
    use wiremock::{matchers, MockServer, Request};

    use claims::{assert_err, assert_ok};
    use wiremock;

    // A struct to use for matching email body - anything that
    // implements Match can be used in the and() or given() methods
    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);

            // Check that all the mandatory fields are populated
            // without inspecting the field values
            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                false
            }
        }
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        // Arrange
        let mock_server = wiremock::MockServer::start().await; // this is a real server run on a thread!
                                                               // make an email client
        let address = mock_server.uri(); // the address the server is running on

        println!("{}", &address);

        let email_client = email_client(address);

        // give the mock server some parameters by 'mounting' a Mock
        // when the server receives a request it iterates over all Mocks
        // to check if the request matches thier conditions
        wiremock::Mock::given(matchers::header_exists("X-Postmark-Server-Token")) // given specifies the conditions
            .and(matchers::header("Content-Type", "application/json"))
            .and(matchers::path("/email"))
            .and(matchers::method("POST"))
            .and(SendEmailBodyMatcher) // our custom message body checker defined above
            .respond_with(wiremock::ResponseTemplate::new(200)) // normally responds with 404 to everything
            .expect(1) // server should expect 1 request only - this is verfied when the test ends
            .mount(&mock_server) // mounts only work if 'mounted' on the mock server
            .await;

        // Act
        let _ = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        // Assert
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        wiremock::Mock::given(matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(200)) // server responds with a 200
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        wiremock::Mock::given(matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(500)) // server responds with a 500
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;

        let email_client = email_client(mock_server.uri());

        let response =
            wiremock::ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(180)); // a long delay before responding

        wiremock::Mock::given(matchers::any())
            .respond_with(response) // server responds with a 200 after a long delay
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(&email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome); // we want this to err
    }

    // Generate a random email subject
    fn subject() -> String {
        Sentence(1..2).fake()
    }
    // Generate a random email content
    fn content() -> String {
        Paragraph(1..10).fake()
    }
    // Generate a random subscriber email
    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }
    /// Get a test instance of `EmailClient`.
    fn email_client(base_url: String) -> EmailClient {
        let timeout = std::time::Duration::from_millis(200);
        EmailClient::new(base_url, email(), Secret::new(Faker.fake()), timeout)
    }
}

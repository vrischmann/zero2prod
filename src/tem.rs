use crate::domain::SubscriberEmail;
use secrecy::{ExposeSecret, Secret};
use std::time::Duration;

#[derive(Clone, serde::Serialize)]
pub struct ProjectId(String);

impl ProjectId {
    pub fn new(s: String) -> Self {
        Self(s)
    }
}

#[derive(serde::Serialize)]
struct SendEmailRequestRecipient<'a> {
    email: &'a str,
    name: Option<&'a str>,
}

#[derive(serde::Serialize)]
struct SendEmailRequest<'a> {
    from: SendEmailRequestRecipient<'a>,
    to: Vec<SendEmailRequestRecipient<'a>>,
    subject: String,
    text: String,
    html: String,
    project_id: ProjectId,
}

pub struct Client {
    http_client: reqwest::Client,

    base_url: String,
    project_id: ProjectId,
    auth_key: Secret<String>,
    sender: SubscriberEmail,
}

impl Client {
    pub fn new(
        base_url: String,
        project_id: ProjectId,
        auth_key: Secret<String>,
        sender: SubscriberEmail,
        timeout: Duration,
    ) -> Self {
        let http_client = reqwest::Client::builder().timeout(timeout).build().unwrap();

        Self {
            http_client,
            base_url,
            project_id,
            auth_key,
            sender,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/emails", &self.base_url);

        let body = SendEmailRequest {
            from: SendEmailRequestRecipient {
                email: self.sender.as_ref(),
                name: Some("Vincent"),
            },
            to: vec![SendEmailRequestRecipient {
                email: recipient.as_ref(),
                name: None,
            }],
            project_id: self.project_id.clone(),
            subject: subject.to_string(),
            text: text_content.to_string(),
            html: html_content.to_string(),
        };

        self.http_client
            .post(url)
            .header("X-Auth-Token", self.auth_key.expose_secret())
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Client;
    use super::ProjectId;
    use crate::domain::SubscriberEmail;
    use claim::{assert_err, assert_ok};
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use std::time::Duration;
    use uuid::Uuid;
    use wiremock::matchers::{any, header, header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &wiremock::Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("from").is_some()
                    && body.get("to").is_some()
                    && body.get("project_id").is_some()
                    && body.get("subject").is_some()
                    && body.get("text").is_some()
                    && body.get("html").is_some()
            } else {
                false
            }
        }
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn email_client(base_url: String) -> Client {
        let project_id = ProjectId(Uuid::new_v4().to_string());

        Client::new(
            base_url,
            project_id,
            Secret::new(Faker.fake()),
            email(),
            Duration::from_millis(100),
        )
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        let mock_server = MockServer::start().await;
        let client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_ok!(result);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        let client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(result);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let client = email_client(mock_server.uri());

        let response = ResponseTemplate::new(200).set_delay(Duration::from_secs(180));

        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(result);
    }

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        let mock_server = MockServer::start().await;
        let client = email_client(mock_server.uri());

        Mock::given(header_exists("X-Auth-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/emails"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = client
            .send_email(email(), &subject(), &content(), &content())
            .await;
    }
}

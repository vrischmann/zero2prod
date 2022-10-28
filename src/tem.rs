use crate::domain::SubscriberEmail;
use secrecy::{ExposeSecret, Secret};

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
    ) -> Self {
        Self {
            http_client: reqwest::Client::new(),
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
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Client;
    use super::ProjectId;
    use crate::domain::SubscriberEmail;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use uuid::Uuid;
    use wiremock::matchers::{header, header_exists, method, path};
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

    #[tokio::test]
    async fn send_email_fires_a_request_to_base_url() {
        let mock_server = MockServer::start().await;
        let project_id = ProjectId(Uuid::new_v4().to_string());
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let client = Client::new(
            mock_server.uri(),
            project_id,
            Secret::new(Faker.fake()),
            sender,
        );

        Mock::given(header_exists("X-Auth-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/emails"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        let _ = client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;
    }
}

use crate::helpers::spawn_app;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[derive(serde::Serialize)]
struct Body {
    name: String,
    email: String,
}

impl Body {
    fn encode(&self) -> String {
        serde_urlencoded::to_string(self).expect("Failed to encode body")
    }
}

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let app = spawn_app().await;

    let body = Body {
        name: Name().fake(),
        email: SafeEmail().fake(),
    };

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.encode()).await;

    //

    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!(
        "SELECT email, name FROM subscriptions WHERE email = $1",
        &body.email
    )
    .fetch_one(&app.pool)
    .await
    .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, body.email);
    assert_eq!(saved.name, body.name);
}

#[tokio::test]
async fn subscribe_returns_400_when_data_is_invalid() {
    let app = spawn_app().await;

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
        ("name=&email=ursula_le_guin%40gmail.com", "name is empty"),
        ("name=le%20guin&email=", "email is empty"),
        (
            const_str::concat!(
                "name=",
                const_str::repeat!("a", 300),
                "&email=ursula_le_guin%40gmail.com"
            ),
            "name is too long",
        ),
    ];

    //

    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(invalid_body.into()).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with a 400. failure condition={}",
            error_message
        )
    }
}

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    let app = spawn_app().await;

    let body = Body {
        name: Name().fake(),
        email: SafeEmail().fake(),
    };

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    //

    let _ = app.post_subscriptions(body.encode()).await;
}

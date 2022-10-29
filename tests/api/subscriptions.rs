use crate::helpers::{spawn_app, SubscriptionBody, UrlEncodedBody};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn subscribe_returns_200_for_valid_form_data() {
    let app = spawn_app().await;

    let body = SubscriptionBody {
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
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    let app = spawn_app().await;

    let body = SubscriptionBody {
        name: Name().fake(),
        email: SafeEmail().fake(),
    };

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let _ = app.post_subscriptions(body.encode()).await;

    //

    let saved = sqlx::query!(
        "SELECT email, name, status FROM subscriptions WHERE email = $1",
        &body.email
    )
    .fetch_one(&app.pool)
    .await
    .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, body.email);
    assert_eq!(saved.name, body.name);
    assert_eq!(saved.status, "pending_confirmation");
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

    let body = SubscriptionBody {
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

#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    let app = spawn_app().await;

    let body = SubscriptionBody {
        name: Name().fake(),
        email: SafeEmail().fake(),
    };

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    //

    let _ = app.post_subscriptions(body.encode()).await;

    //

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

    let get_link = |s: &str| -> String {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();

        assert_eq!(links.len(), 1, "expected 1 link, got {}", links.len());

        links[0].as_str().to_owned()
    };

    let html_link = get_link(body["html"].as_str().unwrap());
    let text_link = get_link(body["text"].as_str().unwrap());
    assert_eq!(
        html_link, text_link,
        "html link={} - text link={}",
        html_link, text_link
    );
}

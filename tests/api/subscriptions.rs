use crate::helpers::{spawn_app, spawn_app_with_pool, SubscriptionBody};
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

    let response = app.post_subscriptions(&body).await;

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

    let _ = app.post_subscriptions(&body).await;

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
        (vec![("name", "le guin")], "missing the email"),
        (
            vec![("email", "ursula_le_guin@gmail.com")],
            "missing the name",
        ),
        (vec![("", ""), ("", "")], "missing both name and email"),
        (
            vec![("name", ""), ("email", "ursula_le_guin@gmail.com")],
            "name is empty",
        ),
        (vec![("name", "le guin"), ("email", "")], "email is empty"),
        (
            vec![
                ("name", const_str::repeat!("a", 300)),
                ("email", "ursula_le_guin@gmail.com"),
            ],
            "name is too long",
        ),
    ];

    //

    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(&invalid_body).await;
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

    let _ = app.post_subscriptions(&body).await;
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

    let _ = app.post_subscriptions(&body).await;

    //

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    assert_eq!(
        confirmation_links.html, confirmation_links.text,
        "html link={} - text link={}",
        confirmation_links.html, confirmation_links.text
    );
}

#[sqlx::test]
async fn subscribe_fails_if_there_is_a_fatal_database_error(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    // Sabotage the database
    // sqlx::query!("ALTER TABLE subscription_tokens DROP COLUMN subscription_token")
    sqlx::query!("ALTER TABLE subscriptions DROP COLUMN email")
        .execute(&app.pool)
        .await
        .unwrap();

    let body = SubscriptionBody {
        name: Name().fake(),
        email: SafeEmail().fake(),
    };

    let response = app.post_subscriptions(&body).await;

    assert_eq!(response.status().as_u16(), 500);
}

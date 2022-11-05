use crate::helpers::{spawn_app, SubscriptionBody};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[sqlx::test]
async fn confirmations_without_token_are_rejected_with_a_400(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
}

#[sqlx::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

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
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    //

    let response = reqwest::get(confirmation_links.html).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
}

#[sqlx::test]
async fn clicking_on_confirmation_link_confirms_a_subscriber(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

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
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = app.get_confirmation_links(email_request);

    //

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let saved = sqlx::query!(
        "SELECT email, name, status FROM subscriptions WHERE email = $1",
        &body.email
    )
    .fetch_one(&app.pool)
    .await
    .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, body.email);
    assert_eq!(saved.name, body.name);
    assert_eq!(saved.status, "confirmed");
}

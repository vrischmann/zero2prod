use crate::helpers::{assert_is_redirect_to, spawn_app, spawn_app_with_pool};
use crate::helpers::{ConfirmationLinks, TestApp};
use crate::helpers::{LoginBody, SubmitNewsletterBody, SubscriptionBody};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use std::time::Duration;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[sqlx::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    create_unconfirmed_subscriber(&app).await;

    Mock::given(path("/emails"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    // Send once
    let newsletter_request_body = SubmitNewsletterBody {
        title: "Newsletter title".to_string(),
        text_content: "Newsletter body as plain text".to_string(),
        html_content: "<p>Newsletter body as HTML</p>".to_string(),
        idempotency_key: Uuid::new_v4(),
    };

    let response = app.post_admin_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    app.dispatch_all_pending_emails().await;

    // Mock verifies on Drop that we haven't sent the newsletter email
}

#[sqlx::test]
async fn newsletters_are_delivered_to_confirmed_subscribers(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    create_confirmed_subscriber(&app).await;

    Mock::given(path("/emails"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    // Send once
    let newsletter_request_body = SubmitNewsletterBody {
        title: "Newsletter title".to_string(),
        text_content: "Newsletter body as plain text".to_string(),
        html_content: "<p>Newsletter body as HTML</p>".to_string(),
        idempotency_key: Uuid::new_v4(),
    };

    let response = app.post_admin_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // Reload the page
    let html_page = app.get_admin_newsletters_html().await;
    assert!(html_page.contains(r#"The newsletter issue has been published"#));

    app.dispatch_all_pending_emails().await;

    // Mock verifies on Drop that we have sent the newsletter email
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    //

    let test_cases = vec![
        (
            SubmitNewsletterBody {
                title: "".to_string(),
                text_content: "Text".to_string(),
                html_content: "HTML".to_string(),
                idempotency_key: Uuid::new_v4(),
            },
            "missing title",
        ),
        (
            SubmitNewsletterBody {
                title: "My title".to_string(),
                text_content: "".to_string(),
                html_content: "".to_string(),
                idempotency_key: Uuid::new_v4(),
            },
            "missing content",
        ),
    ];

    for (invalid_body, case) in test_cases {
        let response = app.post_admin_newsletters(&invalid_body).await;

        let response_status = response.status();
        let _response_body = response.text().await.expect("Failed to get response body");

        assert_eq!(
            response_status, 400,
            "The API did not fail with a 400 Bad Request for case '{}'",
            case,
        )
    }
}

#[sqlx::test]
async fn newsletter_creation_is_idempotent(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    create_confirmed_subscriber(&app).await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    //

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount(&app.email_server)
        .await;

    // 1) Send once
    let newsletter_request_body = SubmitNewsletterBody {
        title: "Newsletter title".to_string(),
        text_content: "Newsletter body as plain text".to_string(),
        html_content: "<p>Newsletter bo as HTML</p>".to_string(),
        idempotency_key: Uuid::new_v4(),
    };

    let response = app.post_admin_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // 2) Reload the page
    let html_page = app.get_admin_newsletters_html().await;
    assert!(html_page.contains(r#"The newsletter issue has been published"#));

    // 3) Send again
    let response = app.post_admin_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    // 4) Reload the page _again_
    let html_page = app.get_admin_newsletters_html().await;
    assert!(html_page.contains(r#"The newsletter issue has been published"#));

    app.dispatch_all_pending_emails().await;

    // Mock verifies on Drop that we have sent the newsletter email _once_
}

#[sqlx::test]
async fn newsletter_creation_concurrent_form_submission_is_handled_gracefully(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    create_confirmed_subscriber(&app).await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    //

    Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // 1) Send once
    let newsletter_request_body = SubmitNewsletterBody {
        title: "Newsletter title".to_string(),
        text_content: "Newsletter body as plain text".to_string(),
        html_content: "<p>Newsletter bo as HTML</p>".to_string(),
        idempotency_key: Uuid::new_v4(),
    };

    let (response1, response2) = tokio::join!(
        app.post_admin_newsletters(&newsletter_request_body),
        app.post_admin_newsletters(&newsletter_request_body),
    );

    assert_is_redirect_to(&response1, "/admin/newsletters");
    assert_is_redirect_to(&response2, "/admin/newsletters");

    app.dispatch_all_pending_emails().await;

    // Mock verifies on Drop that we have sent the newsletter email _once_
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = SubscriptionBody {
        name: Name().fake(),
        email: SafeEmail().fake(),
    };

    let _mock_guard = Mock::given(path("/emails"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(&body)
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_links = create_unconfirmed_subscriber(app).await;

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

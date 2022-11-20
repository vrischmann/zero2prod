use crate::helpers::{
    spawn_app, spawn_app_with_pool, ConfirmationLinks, LoginBody, NewsletterContent,
    SubmitNewsletterBody, SubscriptionBody, TestApp,
};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[sqlx::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    //

    let newsletter_request_body = SubmitNewsletterBody {
        title: "Newsletter title".to_string(),
        content: NewsletterContent {
            text: "Newsletter body as plain text".to_string(),
            html: "<p>Newsletter body as HTML</p>".to_string(),
        },
    };

    let response = app.post_admin_newsletters(&newsletter_request_body).await;
    assert_eq!(response.status().as_u16(), 200);
}

#[sqlx::test]
async fn newsletters_are_delivered_to_confirmed_subscribers(pool: sqlx::PgPool) {
    let app = spawn_app_with_pool(pool).await;

    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_login(&LoginBody {
        username: app.test_user.username.clone(),
        password: app.test_user.password.clone(),
    })
    .await;

    //

    let newsletter_request_body = SubmitNewsletterBody {
        title: "Newsletter title".to_string(),
        content: NewsletterContent {
            text: "Newsletter body as plain text".to_string(),
            html: "<p>Newsletter body as HTML</p>".to_string(),
        },
    };

    let response = app.post_admin_newsletters(&newsletter_request_body).await;
    assert_eq!(response.status().as_u16(), 200);
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
                content: NewsletterContent {
                    text: "Text".to_string(),
                    html: "HTML".to_string(),
                },
            },
            "missing title",
        ),
        (
            SubmitNewsletterBody {
                title: "My title".to_string(),
                content: NewsletterContent {
                    text: "".to_string(),
                    html: "".to_string(),
                },
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

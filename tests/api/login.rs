use crate::helpers::{assert_is_redirect_to, spawn_app, LoginBody};

#[sqlx::test]
async fn an_error_flash_message_is_set_on_failure(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

    // 1) wrong credentials, expect a flash message

    let body = LoginBody {
        username: "random-username".to_string(),
        password: "random-password".to_string(),
    };

    let response = app.post_login(&body).await;

    assert_is_redirect_to(&response, "/login");

    const EXPECTED_HTML: &str = r#"<p class="flash flash-error"><i>Authentication failed</i></p>"#;

    // 2) reload the page to check that the handlers prints the flash message
    let html_page = app.get_login_html().await;
    assert!(html_page.contains(EXPECTED_HTML));

    // 3) reload the page once again; now we don't expect the handlers to print the flash message
    let html_page = app.get_login_html().await;
    assert!(!html_page.contains(EXPECTED_HTML));
}

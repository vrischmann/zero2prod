use crate::helpers::{assert_is_redirect_to, spawn_app, LoginBody};

#[sqlx::test]
async fn an_error_flash_message_is_set_on_failure(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

    let body = LoginBody {
        username: "random-username".to_string(),
        password: "random-password".to_string(),
    };

    let response = app.post_login(&body).await;

    assert_is_redirect_to(&response, "/login");

    let flash_cookie = response.cookies().find(|c| c.name() == "_flash").unwrap();
    assert_eq!(flash_cookie.value(), "Authentication failed");
}

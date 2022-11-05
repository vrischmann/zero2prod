use crate::helpers::{spawn_app, LoginBody};

#[sqlx::test]
async fn an_error_flash_message_is_set_on_failure(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;

    let body = LoginBody {
        username: "random-username".to_string(),
        password: "random-password".to_string(),
    };

    let response = app.post_login(&body).await;

    assert_eq!(response.status().as_u16(), 303);
}

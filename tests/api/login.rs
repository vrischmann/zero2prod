use crate::helpers::{spawn_app, TestApp};

#[sqlx::test]
async fn an_error_flash_message_is_set_on_failure(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
}

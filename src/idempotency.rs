use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use anyhow::anyhow;
use uuid::Uuid;

#[derive(Debug)]
pub struct IdempotencyKey(String);

const MAX_KEY_LENGTH: usize = 50;

impl TryFrom<String> for IdempotencyKey {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.is_empty() {
            Err(anyhow!("idempotency key cannot be empty"))
        } else if s.len() >= MAX_KEY_LENGTH {
            Err(anyhow!(
                "idempotency key must be shorter than {} characters",
                MAX_KEY_LENGTH
            ))
        } else {
            Ok(Self(s))
        }
    }
}

impl From<IdempotencyKey> for String {
    fn from(key: IdempotencyKey) -> Self {
        key.0
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub async fn get_saved_response(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    idempotency_key: &IdempotencyKey,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    #[derive(Debug, sqlx::Type)]
    #[sqlx(type_name = "header_pair")]
    struct HeaderPairRecord {
        name: String,
        value: Vec<u8>,
    }

    let saved_response = sqlx::query!(
        r#"
            SELECT
                response_status_code,
                response_headers as "response_headers: Vec<HeaderPairRecord>",
                response_body
            FROM idempotency
            WHERE user_id = $1 AND idempotency_key = $2
            "#,
        user_id,
        idempotency_key.as_ref(),
    )
    .fetch_optional(pool)
    .await?;

    // Rebuild the response

    if let Some(saved_response) = saved_response {
        let response_status_code = saved_response.response_status_code.try_into()?;
        let status_code = StatusCode::from_u16(response_status_code)?;

        let mut response_builder = HttpResponse::build(status_code);
        for HeaderPairRecord { name, value } in saved_response.response_headers {
            response_builder.append_header((name, value));
        }

        let response = response_builder.body(saved_response.response_body);

        Ok(Some(response))
    } else {
        Ok(None)
    }
}

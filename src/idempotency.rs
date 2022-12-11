use actix_web::body;
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

#[derive(Debug, sqlx::Type)]
#[sqlx(type_name = "header_pair")]
struct HeaderPairRecord {
    name: String,
    value: Vec<u8>,
}

impl sqlx::postgres::PgHasArrayType for HeaderPairRecord {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("_header_pair")
    }
}

pub async fn get_saved_response(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    idempotency_key: &IdempotencyKey,
) -> Result<Option<HttpResponse>, anyhow::Error> {
    let saved_response = sqlx::query!(
        r#"
            SELECT
                response_status_code as "response_status_code!",
                response_headers as "response_headers!: Vec<HeaderPairRecord>",
                response_body as "response_body!"
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

pub async fn save_response(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    idempotency_key: &IdempotencyKey,
    http_response: HttpResponse,
) -> Result<HttpResponse, anyhow::Error> {
    let (response_head, body) = http_response.into_parts();

    let body = body::to_bytes(body).await.map_err(|e| anyhow!("{}", e))?;
    let status_code = response_head.status().as_u16() as i16;

    let headers = {
        let mut v = Vec::with_capacity(response_head.headers().len());
        for (name, value) in response_head.headers().iter() {
            let name = name.as_str().to_owned();
            let value = value.as_bytes().to_owned();

            v.push(HeaderPairRecord { name, value });
        }

        v
    };

    sqlx::query_unchecked!(
        r#"
        INSERT INTO idempotency(
            user_id,
            idempotency_key,
            response_status_code,
            response_headers,
            response_body,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, now())
        "#,
        user_id,
        idempotency_key.as_ref(),
        status_code,
        headers,
        body.as_ref(),
    )
    .execute(pool)
    .await?;

    //

    let new_http_response = response_head.set_body(body).map_into_boxed_body();

    Ok(new_http_response)
}

use anyhow::anyhow;

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

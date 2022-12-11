use crate::domain::SubscriberEmail;
use crate::tem;
use std::time::Duration;
use tracing::error;
use uuid::Uuid;

enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id = tracing::field::Empty,
        subscriber_email = tracing::field::Empty,
    )
)]
async fn try_execute_task(
    pool: &sqlx::PgPool,
    email_client: &tem::Client,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let result = dequeue_task(pool).await?;
    if result.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }

    let (transaction, issue_id, email) = result.unwrap();

    match SubscriberEmail::parse(email.clone()) {
        Ok(email) => {
            let issue = get_issue(pool, issue_id).await?;

            let send_result = email_client
                .send_email(
                    &email,
                    &issue.title,
                    &issue.html_content,
                    &issue.text_content,
                )
                .await;

            if let Err(err) = send_result {
                error!(
                    error.cause_chain = ?err,
                    error.message = %err,
                    "Failed to deliver issue to a confirmed subscriber, skipping",
                )
            }
        }
        Err(err) => {
            error!(
                error.cause_chain = ?err,
                error.message = %err,
                "Skipping a confirmed subscriber, their stored contact details are invalid",
            )
        }
    }

    delete_task(transaction, issue_id, &email).await?;

    Ok(ExecutionOutcome::TaskCompleted)
}

type PgTransaction = sqlx::Transaction<'static, sqlx::Postgres>;

#[tracing::instrument(skip_all)]
async fn dequeue_task(
    pool: &sqlx::PgPool,
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;

    let record = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email
        FROM issue_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut transaction)
    .await?;

    match record {
        Some(record) => {
            let result = (
                transaction,
                record.newsletter_issue_id,
                record.subscriber_email,
            );
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE newsletter_issue_id = $1 AND subscriber_email = $2
        "#,
        issue_id,
        email,
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &sqlx::PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content
        FROM newsletter_issues
        WHERE id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;

    Ok(issue)
}

async fn worker_loop(pool: sqlx::PgPool, email_client: tem::Client) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::TaskCompleted) => {}
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

pub async fn run_worker_until_stopped(
    pool: sqlx::PgPool,
    email_client: tem::Client,
) -> Result<(), anyhow::Error> {
    worker_loop(pool, email_client).await
}

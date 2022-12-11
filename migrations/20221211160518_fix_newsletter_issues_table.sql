ALTER TABLE newsletter_issues RENAME COLUMN html_context TO html_content;
ALTER TABLE newsletter_issues DROP COLUMN published_at;
ALTER TABLE newsletter_issues ADD COLUMN published_at timestamptz NOT NULL;

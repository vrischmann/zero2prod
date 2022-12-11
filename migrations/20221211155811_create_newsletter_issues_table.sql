CREATE TABLE newsletter_issues(
  id uuid NOT NULL,
  title TEXT NOT NULL,
  text_content TEXT NOT NULL,
  html_context TEXT NOT NULL,
  published_at TEXT NOT NULL,
  PRIMARY KEY (id)
);

CREATE TABLE sessions(
  id uuid PRIMARY KEY,
  state bytea NOT NULL,
  created_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL
);

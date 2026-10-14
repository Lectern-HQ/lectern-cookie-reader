-- The columns of a Chromium cookie database that this crate reads.
CREATE TABLE meta(key TEXT, value TEXT);
CREATE TABLE cookies(
  host_key TEXT, name TEXT, path TEXT, is_secure INTEGER, is_httponly INTEGER,
  has_expires INTEGER, expires_utc INTEGER, last_access_utc INTEGER, encrypted_value BLOB
);

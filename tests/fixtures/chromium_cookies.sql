-- The tables of a Chromium cookie database at schema version 24: the same columns, types
-- and unique index as `CreateV24Schema` in Chromium's
-- net/extras/sqlite/sqlite_persistent_cookie_store.cc, and the `meta` table of
-- sql/meta_table.cc.
CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR);
CREATE TABLE cookies(
  creation_utc INTEGER NOT NULL,
  host_key TEXT NOT NULL,
  top_frame_site_key TEXT NOT NULL,
  name TEXT NOT NULL,
  value TEXT NOT NULL,
  encrypted_value BLOB NOT NULL,
  path TEXT NOT NULL,
  expires_utc INTEGER NOT NULL,
  is_secure INTEGER NOT NULL,
  is_httponly INTEGER NOT NULL,
  last_access_utc INTEGER NOT NULL,
  has_expires INTEGER NOT NULL,
  is_persistent INTEGER NOT NULL,
  priority INTEGER NOT NULL,
  samesite INTEGER NOT NULL,
  source_scheme INTEGER NOT NULL,
  source_port INTEGER NOT NULL,
  last_update_utc INTEGER NOT NULL,
  source_type INTEGER NOT NULL,
  has_cross_site_ancestor INTEGER NOT NULL
);
CREATE UNIQUE INDEX cookies_unique_index ON cookies(
  host_key, top_frame_site_key, has_cross_site_ancestor, name, path, source_scheme,
  source_port
);

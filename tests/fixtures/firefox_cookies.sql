-- The columns of a Firefox family cookie database that this crate reads.
CREATE TABLE moz_cookies(
  id INTEGER PRIMARY KEY, originAttributes TEXT NOT NULL DEFAULT '', name TEXT,
  value TEXT, host TEXT, path TEXT, expiry INTEGER, lastAccessed INTEGER,
  isSecure INTEGER, isHttpOnly INTEGER
);

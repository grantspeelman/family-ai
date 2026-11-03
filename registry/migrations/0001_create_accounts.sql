-- `id` is the identity; an external subject id can take over from it later
-- without touching `email`. AUTOINCREMENT means an id is never used twice.
--
-- `email` is kept exactly as given. `email_key` is its case-folded form, set
-- by the store, and is what makes an email unique: SQLite's NOCASE collation
-- folds ASCII letters only.
CREATE TABLE accounts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    email       TEXT NOT NULL,
    email_key   TEXT NOT NULL UNIQUE,
    family_name TEXT NOT NULL
) STRICT;

-- The one table the round-trip endpoint writes to and reads from.
CREATE TABLE notes (
    id INTEGER PRIMARY KEY,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

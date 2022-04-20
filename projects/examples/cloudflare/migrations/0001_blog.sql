-- Mirrors `schemas/blog.iris` for D1 (SQLite). Prefer `iris push` long-term when D1 session lands.
CREATE TABLE IF NOT EXISTS User (
    user_id TEXT PRIMARY KEY NOT NULL,
    user_name TEXT NOT NULL,
    active INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS Post (
    post_id TEXT PRIMARY KEY NOT NULL,
    author TEXT NOT NULL,
    title TEXT NOT NULL,
    published INTEGER NOT NULL,
    FOREIGN KEY (author) REFERENCES User(user_id)
);

CREATE INDEX IF NOT EXISTS idx_post_author ON Post(author);
CREATE INDEX IF NOT EXISTS idx_post_published ON Post(published);

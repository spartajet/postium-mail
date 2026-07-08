PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    display_name TEXT,
    provider TEXT NOT NULL,
    imap_host TEXT,
    imap_port INTEGER,
    imap_ssl INTEGER DEFAULT 1,
    imap_ssl_mode TEXT,
    smtp_host TEXT,
    smtp_port INTEGER,
    smtp_ssl INTEGER DEFAULT 1,
    smtp_ssl_mode TEXT,
    color TEXT,
    sync_enabled INTEGER DEFAULT 1,
    last_sync_at INTEGER,
    auth_type TEXT DEFAULT 'password',
    account_type TEXT NOT NULL DEFAULT 'personal',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS emails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    message_id TEXT,
    subject TEXT,
    sender_name TEXT,
    sender_email TEXT NOT NULL,
    recipient_emails TEXT NOT NULL,
    cc_emails TEXT,
    bcc_emails TEXT,
    preview TEXT,
    body_text TEXT,
    body_html TEXT,
    is_read INTEGER DEFAULT 0,
    is_starred INTEGER DEFAULT 0,
    is_draft INTEGER DEFAULT 0,
    is_answered INTEGER DEFAULT 0,
    is_deleted INTEGER DEFAULT 0,
    sent_at INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email_id INTEGER NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
    filename TEXT,
    content_type TEXT,
    size INTEGER NOT NULL,
    section_path TEXT NOT NULL,
    disposition TEXT,
    content_id TEXT,
    path TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS email_labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email_id INTEGER NOT NULL REFERENCES emails(id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    UNIQUE(email_id, label_id)
);

CREATE TABLE IF NOT EXISTS sync_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder TEXT NOT NULL,
    folder_category TEXT,
    folder_nick_name TEXT,
    uidvalidity INTEGER,
    uidnext INTEGER,
    synced_at INTEGER,
    last_sync_uid INTEGER,
    history_synced_since INTEGER,
    history_before_uid INTEGER,
    history_exhausted INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE IF NOT EXISTS sync_errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder TEXT,
    error_type TEXT NOT NULL,
    error_message TEXT NOT NULL,
    uid INTEGER,
    stack_trace TEXT,
    resolved INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_emails_account ON emails(account_id);
CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder);
CREATE INDEX IF NOT EXISTS idx_emails_sent_at ON emails(sent_at DESC);
CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read);
CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_account_folder_uid ON emails(account_id, folder, uid);
CREATE INDEX IF NOT EXISTS idx_attachments_email ON attachments(email_id);
CREATE INDEX IF NOT EXISTS idx_labels_account ON labels(account_id);
CREATE INDEX IF NOT EXISTS idx_email_labels_email ON email_labels(email_id);
CREATE INDEX IF NOT EXISTS idx_email_labels_label ON email_labels(label_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_state_account_folder ON sync_state(account_id, folder);
CREATE INDEX IF NOT EXISTS idx_sync_errors_account ON sync_errors(account_id);

CREATE VIRTUAL TABLE IF NOT EXISTS emails_fts USING fts5(
    subject,
    sender_email,
    preview,
    content='emails',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS emails_fts_ai AFTER INSERT ON emails BEGIN
    INSERT INTO emails_fts(rowid, subject, sender_email, preview)
    VALUES (new.id, new.subject, new.sender_email, new.preview);
END;

CREATE TRIGGER IF NOT EXISTS emails_fts_ad AFTER DELETE ON emails BEGIN
    INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
    VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
END;

CREATE TRIGGER IF NOT EXISTS emails_fts_au AFTER UPDATE ON emails BEGIN
    INSERT INTO emails_fts(emails_fts, rowid, subject, sender_email, preview)
    VALUES ('delete', old.id, old.subject, old.sender_email, old.preview);
    INSERT INTO emails_fts(rowid, subject, sender_email, preview)
    VALUES (new.id, new.subject, new.sender_email, new.preview);
END;

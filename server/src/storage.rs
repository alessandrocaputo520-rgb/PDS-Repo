use rusqlite::{params, Connection, Result};

use common::models::{Position, User};

// ── Inizializzazione ──────────────────────────────────────────────────────────

/// Apre (o crea) il database SQLite e inizializza lo schema.
pub fn init_db(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT    NOT NULL UNIQUE,
            password_hash TEXT    NOT NULL,
            created_at    INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS positions (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id   INTEGER NOT NULL,
            lat       REAL    NOT NULL,
            lon       REAL    NOT NULL,
            timestamp INTEGER NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id)
        );

        CREATE INDEX IF NOT EXISTS idx_pos_user_time
            ON positions(user_id, timestamp);

        CREATE TABLE IF NOT EXISTS messages (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            from_user_id INTEGER,
            to_user_id   INTEGER,
            content      TEXT    NOT NULL,
            timestamp    INTEGER NOT NULL
        );
        ",
    )?;
    Ok(conn)
}

// ── Utenti ────────────────────────────────────────────────────────────────────

pub fn insert_user(conn: &Connection, username: &str, password_hash: &str) -> Result<i64> {
    let now = now_secs();
    conn.execute(
        "INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
        params![username, password_hash, now],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn find_user_by_username(conn: &Connection, username: &str) -> Result<Option<User>> {
    let mut stmt = conn.prepare(
        "SELECT id, username, password_hash, created_at FROM users WHERE username = ?1",
    )?;
    let mut rows = stmt.query(params![username])?;
    if let Some(row) = rows.next()? {
        Ok(Some(User {
            id: row.get(0)?,
            username: row.get(1)?,
            password_hash: row.get(2)?,
            created_at: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn find_user_id_by_username(conn: &Connection, username: &str) -> Result<Option<i64>> {
    let mut stmt =
        conn.prepare("SELECT id FROM users WHERE username = ?1")?;
    let mut rows = stmt.query(params![username])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn get_all_users(conn: &Connection) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare("SELECT id, username FROM users ORDER BY username")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

// ── Posizioni ─────────────────────────────────────────────────────────────────

pub fn insert_position(
    conn: &Connection,
    user_id: i64,
    lat: f64,
    lon: f64,
    timestamp: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO positions (user_id, lat, lon, timestamp) VALUES (?1, ?2, ?3, ?4)",
        params![user_id, lat, lon, timestamp],
    )?;
    Ok(())
}

pub fn get_positions_in_range(
    conn: &Connection,
    user_id: i64,
    start: i64,
    end: i64,
) -> Result<Vec<Position>> {
    let mut stmt = conn.prepare(
        "SELECT user_id, lat, lon, timestamp
           FROM positions
          WHERE user_id = ?1 AND timestamp >= ?2 AND timestamp <= ?3
          ORDER BY timestamp ASC",
    )?;
    let rows = stmt.query_map(params![user_id, start, end], |row| {
        Ok(Position {
            user_id: row.get(0)?,
            lat: row.get(1)?,
            lon: row.get(2)?,
            timestamp: row.get(3)?,
        })
    })?;
    rows.collect()
}

// ── Messaggi ──────────────────────────────────────────────────────────────────

pub fn insert_message(
    conn: &Connection,
    from_user_id: Option<i64>,
    to_user_id: Option<i64>,
    content: &str,
) -> Result<i64> {
    let now = now_secs();
    conn.execute(
        "INSERT INTO messages (from_user_id, to_user_id, content, timestamp) VALUES (?1, ?2, ?3, ?4)",
        params![from_user_id, to_user_id, content, now],
    )?;
    Ok(conn.last_insert_rowid())
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

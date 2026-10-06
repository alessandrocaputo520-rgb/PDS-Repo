//! Persistenza SQLite: registrazioni, campioni GPS e messaggi.
use common::models::{Message, Position, User};
use rusqlite::{params, Connection, OptionalExtension, Result};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS users (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                 password_hash TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS positions (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 user_id INTEGER NOT NULL REFERENCES users(id),
                 lat REAL NOT NULL, lon REAL NOT NULL, timestamp INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_positions_user_time
                 ON positions(user_id, timestamp);
             CREATE TABLE IF NOT EXISTS messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 from_user_id INTEGER REFERENCES users(id),
                 to_user_id INTEGER REFERENCES users(id),
                 content TEXT NOT NULL, timestamp INTEGER NOT NULL,
                 delivered_at INTEGER
             );",
        )?;
        // Migrazione idempotente anche per eventuali DB creati da versioni iniziali.
        let has_delivery: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('messages') WHERE name='delivered_at'",
            [],
            |r| r.get(0),
        )?;
        if !has_delivery {
            conn.execute("ALTER TABLE messages ADD COLUMN delivered_at INTEGER", [])?;
        }
        Ok(Self { conn })
    }

    pub fn add_user(&self, username: &str, password_hash: &str, timestamp: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO users(username, password_hash, created_at) VALUES (?1, ?2, ?3)",
            params![username, password_hash, timestamp],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn find_user(&self, username: &str) -> Result<Option<User>> {
        self.conn.query_row(
            "SELECT id, username, password_hash, created_at FROM users WHERE username=?1 COLLATE NOCASE",
            [username],
            |row| Ok(User {
                id: row.get(0)?, username: row.get(1)?,
                password_hash: row.get(2)?, created_at: row.get(3)?,
            }),
        ).optional()
    }

    pub fn list_users(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, username FROM users ORDER BY username")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect()
    }

    pub fn add_position(&self, p: &Position) -> Result<()> {
        self.conn.execute(
            "INSERT INTO positions(user_id, lat, lon, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![p.user_id, p.lat, p.lon, p.timestamp],
        )?;
        Ok(())
    }

    pub fn latest_position(&self, user_id: i64) -> Result<Option<Position>> {
        self.conn
            .query_row(
                "SELECT user_id, lat, lon, timestamp FROM positions
             WHERE user_id=?1 ORDER BY timestamp DESC, id DESC LIMIT 1",
                [user_id],
                |r| {
                    Ok(Position {
                        user_id: r.get(0)?,
                        lat: r.get(1)?,
                        lon: r.get(2)?,
                        timestamp: r.get(3)?,
                    })
                },
            )
            .optional()
    }

    pub fn positions(&self, user_id: i64, since: i64, until: i64) -> Result<Vec<Position>> {
        let mut stmt = self.conn.prepare(
            "SELECT user_id, lat, lon, timestamp FROM positions
             WHERE user_id=?1 AND timestamp>=?2 AND timestamp<=?3
             ORDER BY timestamp ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![user_id, since, until], |r| {
            Ok(Position {
                user_id: r.get(0)?,
                lat: r.get(1)?,
                lon: r.get(2)?,
                timestamp: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_message(
        &self,
        from: Option<i64>,
        to: Option<i64>,
        content: &str,
        timestamp: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO messages(from_user_id, to_user_id, content, timestamp)
             VALUES (?1, ?2, ?3, ?4)",
            params![from, to, content, timestamp],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn undelivered(&self, user_id: i64) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_user_id, to_user_id, content, timestamp FROM messages
             WHERE to_user_id=?1 AND delivered_at IS NULL ORDER BY timestamp, id",
        )?;
        let rows = stmt.query_map([user_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                from_user_id: r.get(1)?,
                to_user_id: r.get(2)?,
                content: r.get(3)?,
                timestamp: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn mark_delivered(&self, message_id: i64, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET delivered_at=?1 WHERE id=?2",
            params![now, message_id],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn messages(&self, user_id: i64) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_user_id, to_user_id, content, timestamp FROM messages
             WHERE to_user_id IS NULL OR to_user_id=?1 OR from_user_id=?1
             ORDER BY timestamp, id",
        )?;
        let rows = stmt.query_map([user_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                from_user_id: r.get(1)?,
                to_user_id: r.get(2)?,
                content: r.get(3)?,
                timestamp: r.get(4)?,
            })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn database_round_trip() {
        let db = Storage::open(":memory:").unwrap();
        let id = db.add_user("alice", "hash", 100).unwrap();
        assert_eq!(db.find_user("ALICE").unwrap().unwrap().id, id);
        assert!(db.add_user("Alice", "hash", 101).is_err());
        db.add_position(&Position {
            user_id: id,
            lat: 45.0,
            lon: 7.0,
            timestamp: 123,
        })
        .unwrap();
        assert_eq!(db.positions(id, 100, 200).unwrap().len(), 1);
        db.add_message(Some(id), None, "ciao", 125).unwrap();
        assert_eq!(db.messages(id).unwrap()[0].content, "ciao");
        let id_msg = db.add_message(None, Some(id), "offline", 126).unwrap();
        assert_eq!(db.undelivered(id).unwrap().len(), 1);
        db.mark_delivered(id_msg, 127).unwrap();
        assert!(db.undelivered(id).unwrap().is_empty());
    }
}

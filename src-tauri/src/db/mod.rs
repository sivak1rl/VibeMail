pub mod models;
pub mod queries;
pub(crate) mod schema;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        // WAL mode for concurrent reads and crash safety
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let mut db = Self { conn };
        schema::run_migrations(&mut db.conn)?;
        Ok(db)
    }
}

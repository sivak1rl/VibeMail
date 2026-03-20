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
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-32000;
             PRAGMA temp_store=MEMORY;
             PRAGMA mmap_size=268435456;",
        )?;
        let mut db = Self { conn };
        schema::run_migrations(&mut db.conn)?;
        Ok(db)
    }
}

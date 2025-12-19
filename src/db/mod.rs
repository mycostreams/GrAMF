pub mod schema;
pub mod time_series;
pub mod time_series_store;

use rusqlite::Connection;
use std::path::Path;

pub fn open_db(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        "#,
    )?;

    Ok(conn)
}

use rusqlite::Connection;
use std::path::Path;


pub fn open_default_path_db() -> rusqlite::Result<Connection> {
    let data_dir = dirs_next::data_dir().unwrap().join("grAMF");
    std::fs::create_dir_all(&data_dir).unwrap();
    let path = &data_dir.join("gramf.db");
    open_db(path)
}

pub fn open_memory_db() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        "#,
    )?;

    Ok(conn)
}

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

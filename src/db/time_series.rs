use crate::graph_model::types::TimeSeries;
use rusqlite::Connection;

/// Inserts a time series into the database for a given edge ID.
pub fn insert_timeseries<T: serde::Serialize>(
    conn: &Connection,
    edge_id: isize,
    ts: &TimeSeries<T>,
) -> anyhow::Result<()> {
    let blob = postcard::to_allocvec(ts)?;

    conn.execute(
        "INSERT OR REPLACE INTO edge_timeseries (edge_id, data)
         VALUES (?, ?)",
        (edge_id, blob),
    )?;

    Ok(())
}

/// Loads a time series from the database for a given edge ID.
pub fn load_timeseries<T: serde::de::DeserializeOwned>(
    conn: &Connection,
    edge_id: isize,
) -> anyhow::Result<Option<TimeSeries<T>>> {
    let mut stmt = conn.prepare("SELECT data FROM edge_timeseries WHERE edge_id = ?")?;

    let mut rows = stmt.query([edge_id])?;

    if let Some(row) = rows.next()? {
        let blob: Vec<u8> = row.get(0)?;
        let ts: TimeSeries<T> = postcard::from_bytes(&blob)?;
        Ok(Some(ts))
    } else {
        Ok(None)
    }
}

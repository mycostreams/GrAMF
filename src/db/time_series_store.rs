use std::num::NonZero;

use lru::LruCache;
use rusqlite::Connection;

use crate::{graph_model::types::TimeSeries};

/// A store for time series data associated with edges in a graph.
/// It uses an LRU cache to minimize database reads.
pub struct TimeSeriesStore<T> {
    conn: Connection,
    cache: LruCache<isize, TimeSeries<T>>,
}

/// Implementations for TimeSeriesStore
impl<T> TimeSeriesStore<T>
where
    T: serde::de::DeserializeOwned,
{
    /// Creates a new TimeSeriesStore with the given database connection and cache capacity.
    pub fn new(conn: Connection, capacity: usize) -> Self {
        Self {
            conn,
            cache: LruCache::new(NonZero::new(capacity).unwrap()),
        }
    }

    /// Retrieves the time series data for the given edge ID.
    pub fn get(&mut self, edge_id: isize) -> anyhow::Result<Option<&TimeSeries<T>>> {
        // Check if the time series is in the cache
        if self.cache.contains(&edge_id) {
            return Ok(self.cache.get(&edge_id));
        }

        // Load the time series from the database
        // let ts = load_timeseries(&self.conn, edge_id)?;

        // // Insert into cache if found
        // if let Some(ts) = ts {
        //     self.cache.put(edge_id, ts);
        // }

        // Return the time series from the cache
        Ok(self.cache.get(&edge_id))
    }
}

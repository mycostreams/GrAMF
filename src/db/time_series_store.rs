use std::num::NonZero;

use lru::LruCache;
use rusqlite::Connection;

use crate::{db::time_series::load_timeseries, graph_model::types::TimeSeries};

pub struct TimeSeriesStore<T> {
    conn: Connection,
    cache: LruCache<u64, TimeSeries<T>>,
}

impl<T> TimeSeriesStore<T>
where
    T: serde::de::DeserializeOwned,
{
    pub fn new(conn: Connection, capacity: usize) -> Self {
        Self {
            conn,
            cache: LruCache::new(NonZero::new(capacity).unwrap()),
        }
    }

    pub fn get(&mut self, edge_id: u64) -> anyhow::Result<Option<&TimeSeries<T>>> {
        if self.cache.contains(&edge_id) {
            return Ok(self.cache.get(&edge_id));
        }

        let ts = load_timeseries(&self.conn, edge_id)?;
        if let Some(ts) = ts {
            self.cache.put(edge_id, ts);
        }

        Ok(self.cache.get(&edge_id))
    }
}

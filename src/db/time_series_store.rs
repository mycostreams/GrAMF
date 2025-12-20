use std::num::NonZero;

use lru::LruCache;
use rusqlite::Connection;

use crate::{db::time_series::load_timeseries, graph_model::types::TimeSeries};

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
        let ts = load_timeseries(&self.conn, edge_id)?;

        // Insert into cache if found
        if let Some(ts) = ts {
            self.cache.put(edge_id, ts);
        }

        // Return the time series from the cache
        Ok(self.cache.get(&edge_id))
    }
}

#[test]
fn test_time_series_store() {
    use crate::{
        db::{init_db::open_default_path_db, schema::init_schema, time_series::insert_timeseries},
        graph_model::types::TimeSeries,
    };

    // Setup in-memory database
    let conn = open_default_path_db().unwrap();
    init_schema(&conn).unwrap();
    // Insert sample time series data
    let sample_data = TimeSeries::<f64> {
        timestamps: vec![1.0, 2.0, 3.0],
    };
    insert_timeseries(&conn, 0, &sample_data).unwrap();

    // Create TimeSeriesStore
    let mut store = TimeSeriesStore::<f64>::new(conn, 2);

    // Retrieve time series data
    let ts = store.get(0).unwrap();
    assert!(ts.is_some());
    assert_eq!(ts.unwrap().timestamps, vec![1.0, 2.0, 3.0]);
}

#[test]
fn test_time_series_load_directly() {
    use crate::{
        db::{
            init_db::open_default_path_db,
            schema::init_schema,
            time_series::{insert_timeseries, load_timeseries},
        },
        graph_model::types::TimeSeries,
    };

    // Setup in-memory database
    let conn = open_default_path_db().unwrap();
    init_schema(&conn).unwrap();
    // Insert sample time series data
    let sample_data = TimeSeries::<f64> {
        timestamps: vec![4.0, 5.0, 6.0],
    };
    insert_timeseries(&conn, 1, &sample_data).unwrap();

    // Load time series data directly
    let ts = load_timeseries::<f64>(&conn, 1).unwrap();
    assert!(ts.is_some());
    assert_eq!(ts.unwrap().timestamps, vec![4.0, 5.0, 6.0]);
}

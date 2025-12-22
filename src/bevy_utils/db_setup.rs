use std::io::Error;

use crate::{
    bevy_utils::db_usage::{DbRequestEvent, DbResponseEvent, DbWorker},
    graph_model::{
        edges::{EdgeFull, EdgeTemporals},
        types::TimeSeries,
    },
};
use crossbeam_channel::Sender;
use crossbeam_channel::{Receiver, unbounded};
use rusqlite::{Connection, params_from_iter};

/// Generally used service linked to the database.
pub(crate) struct DbService {
    conn: Connection,
}

enum RequestHandleResult {
    Success,
    Shutdown,
}

/// Opens (or creates) a filepath at the default application data path.
fn open_default_path_db() -> String {
    let data_dir = dirs_next::data_dir().unwrap().join("grAMF");
    std::fs::create_dir_all(&data_dir).unwrap();
    let binding = data_dir.join("gramf.db");
    let data_path = binding.to_str().expect("REASON");
    data_path.to_string()
}

/// Creation of the database, should only be called at the start of the app.
/// Continually runs during the app, consuming request events.
pub(super) fn run_db_service(rx_req: Receiver<DbRequestEvent>, tx_res: Sender<DbResponseEvent>) {
    let mut service = DbService::new(&open_default_path_db()).unwrap();

    let schema_result = service.init_schema();
    match schema_result {
        Ok(_) => (),
        Err(schema_error) => println!("{:?}", schema_error.to_string()),
    }

    while let Ok(request) = rx_req.recv() {
        match handle_request(request, &mut service, &tx_res) {
            Ok(result_val) => match result_val {
                RequestHandleResult::Success => continue,
                RequestHandleResult::Shutdown => break,
            },
            Err(_) => break,
        }
    }
}

/// Middle-man function to select a method based on event type.
fn handle_request(
    request: DbRequestEvent,
    service: &mut DbService,
    tx_res: &Sender<DbResponseEvent>,
) -> Result<RequestHandleResult, Error> {
    match request {
        DbRequestEvent::InsertEdges(edges) => {
            let result = service.insert_edges(edges).unwrap();
            tx_res
                .send(DbResponseEvent::InsertProgress { inserted: result })
                .unwrap();
            Ok(RequestHandleResult::Success)
        }
        DbRequestEvent::QueryEdges(ids) => {
            let results = service.query_edges(ids).unwrap();
            tx_res.send(DbResponseEvent::Edges(results)).unwrap();
            Ok(RequestHandleResult::Success)
        }
        DbRequestEvent::Shutdown => {
            let _ = tx_res.send(DbResponseEvent::ShutdownComplete);
            Ok(RequestHandleResult::Shutdown)
        }
    }
}

impl DbService {
    fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;

                CREATE TABLE IF NOT EXISTS edge (
                    edge_id         INTEGER PRIMARY KEY,
                    source          INTEGER,
                    target          INTEGER,
                    cluster_id      INTEGER,
                    temporal_props  BLOB NOT NULL
                );
                ",
        )
    }

    fn insert_edges(
        &mut self,
        edges: Vec<EdgeFull>,
        // tx_res: &Sender<DbResponseEvent>,
    ) -> Result<usize, rusqlite::Error> {
        let tx = self.conn.transaction()?;

        {
            let mut stmt = tx.prepare(
                "
                    INSERT OR REPLACE INTO edge
                    (edge_id, source, target, cluster_id, temporal_props)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ",
            )?;

            for e in &edges {
                stmt.execute((
                    e.id,
                    e.source,
                    e.target,
                    e.edge_cluster_id,
                    postcard::to_allocvec(&e.temporal_props).unwrap(),
                ))?;
            }
        }

        tx.commit()?;

        // let _ = tx_res.send(DbResponseEvent::InsertProgress {
        //     inserted: edges.len(),
        // });

        Ok(edges.len())
    }

    fn query_edges(
        &self,
        edge_ids: Vec<i64>,
        // tx_res: &Sender<DbResponseEvent>,
    ) -> Result<Vec<EdgeFull>, rusqlite::Error> {
        if edge_ids.is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }

        let placeholders = edge_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql = format!(
            "
            SELECT   edge_id, source, target, cluster_id, temporal_props
            FROM edge
            WHERE edge_id IN ({})
            ",
            placeholders
        );

        let mut stmt = self.conn.prepare(&sql)?;

        let rows = stmt.query_map(params_from_iter(edge_ids.iter()), |row| {
            let blob: Vec<u8> = row.get(4)?;
            let temporal_props: TimeSeries<EdgeTemporals> = postcard::from_bytes(&blob).unwrap();
            Ok(EdgeFull {
                id: row.get(0)?,
                source: row.get(1)?,
                target: row.get(2)?,
                edge_cluster_id: row.get(3)?,
                temporal_props,
            })
        })?;

        let mut results: Vec<EdgeFull> = Vec::new();
        for r in rows {
            results.push(r?);
        }

        Ok(results)
    }
}

#[test]
fn test_database_init() {
    let mut service = DbService::new(&open_default_path_db()).unwrap();

    let _ = service.init_schema().unwrap();

    let trial_data = vec![EdgeFull::new()];

    let return_size = service.insert_edges(trial_data).unwrap();

    assert!(return_size == 1);

    let queried_edges = service.query_edges(vec![1]).unwrap();

    println!("{:?}", queried_edges)
}

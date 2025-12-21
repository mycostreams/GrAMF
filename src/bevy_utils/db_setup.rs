
use crate::{
    bevy_utils::db_usage::{DbRequestEvent, DbResponseEvent},
    graph_model::{
        edges::{EdgeFull, EdgeTemporals},
        types::TimeSeries,
    },
};
use crossbeam_channel::Sender;
use rusqlite::{Connection, params_from_iter};

pub(crate) struct DbService {
    conn: Connection,
}

use crossbeam_channel::Receiver;

enum RequestHandleError {
    Error,
    Shutdown,
}

/// Opens (or creates) a SQLite database at the default application data path.
fn open_default_path_db() -> String {
    let data_dir = dirs_next::data_dir().unwrap().join("grAMF");
    std::fs::create_dir_all(&data_dir).unwrap();
    let binding = data_dir.join("gramf.db");
    let data_path = binding.to_str().expect("REASON");
    data_path.to_string()
}

pub(super) fn run_db_service(rx_req: Receiver<DbRequestEvent>, tx_res: Sender<DbResponseEvent>) {
    let mut service = DbService::new(&open_default_path_db());
    service.init_schema();

    while let Ok(request) = rx_req.recv() {
        match handle_request(request, &mut service, &tx_res) {
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

fn handle_request(request: DbRequestEvent, service: &mut DbService, tx_res: &Sender<DbResponseEvent>) -> Result<(), RequestHandleError> {
    match request {
        DbRequestEvent::InsertEdges(edges) => {
            service.insert_edges(edges, &tx_res);
            Ok(())
        }
        DbRequestEvent::QueryEdges(ids) => {
            service.query_edges(ids, &tx_res);
            Ok(())
        }
        DbRequestEvent::Shutdown => {
            let _ = tx_res.send(DbResponseEvent::ShutdownComplete);
            Err(RequestHandleError::Shutdown)
        }
    }
}

impl DbService {
    fn new(path: &str) -> Self {
        let conn = Connection::open(path).unwrap();
        Self { conn }
    }

    fn init_schema(&self) {
        self.conn
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;

                CREATE TABLE IF NOT EXISTS edge (
                    edge_id         INTEGER PRIMARY KEY,
                    source          INTEGER,
                    target          INTEGER,
                    cluster_id      INTEGER,
                    temporal_props  BLOB NOT NULL,
                );
                ",
            )
            .unwrap();
    }

    fn insert_edges(&mut self, edges: Vec<EdgeFull>, tx_res: &Sender<DbResponseEvent>) {
        let tx = self.conn.transaction().unwrap();

        {
            let mut stmt = tx
                .prepare(
                    "
                    INSERT OR REPLACE INTO edge
                    (edge_id, source, target, cluster_id, temporal_props)
                    VALUES (?1, ?2, ?3, ?4, ?5)
                    ",
                )
                .unwrap();

            for e in &edges {
                // let blob = postcard::to_allocvec(&e.temporal_props)?;
                stmt.execute((
                    e.id,
                    e.source,
                    e.target,
                    e.edge_cluster_id,
                    postcard::to_allocvec(&e.temporal_props).unwrap(),
                ))
                .unwrap();
            }
        }

        tx.commit().unwrap();

        let _ = tx_res.send(DbResponseEvent::InsertProgress {
            inserted: edges.len(),
        });
    }

    fn query_edges(&self, edge_ids: Vec<i64>, tx_res: &Sender<DbResponseEvent>) {
        if edge_ids.is_empty() {
            return;
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

        let mut stmt = self.conn.prepare(&sql).unwrap();

        let rows = stmt
            .query_map(params_from_iter(edge_ids.iter()), |row| {
                let blob: Vec<u8> = row.get(4)?;
                let temporal_props: TimeSeries<EdgeTemporals> =
                    postcard::from_bytes(&blob).unwrap();
                Ok(EdgeFull {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    target: row.get(2)?,
                    edge_cluster_id: row.get(3)?,
                    temporal_props,
                })
            })
            .unwrap();

        let mut results: Vec<EdgeFull> = Vec::new();
        for r in rows {
            results.push(r.unwrap());
        }

        let _ = tx_res.send(DbResponseEvent::Edges(results));
    }
}

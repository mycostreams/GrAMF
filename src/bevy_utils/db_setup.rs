use crate::bevy_utils::db_usage::DbWorker;
use crate::{
    bevy_utils::db_usage::{DbRequestEvent, DbResponseEvent},
    graph_model::{
        edges::{EdgeFull, EdgeTemporals},
        types::TimeSeries,
    },
};
use bevy::ecs::system::Commands;
use crossbeam_channel::Sender;
use rusqlite::{Connection, params_from_iter};

pub(crate) struct DbService {
    conn: Connection,
}

use crossbeam_channel::Receiver;

pub(super) fn run_db_service(rx_req: Receiver<DbRequestEvent>, tx_res: Sender<DbResponseEvent>) {
    let mut service = DbService::new("edges.db");
    service.init_schema();

    for request in rx_req {
        match request {
            DbRequestEvent::InsertEdges(edges) => {
                service.insert_edges(edges, &tx_res);
            }
            DbRequestEvent::QueryEdges(ids) => {
                service.query_edges(ids, &tx_res);
            }
            DbRequestEvent::Shutdown => {
                let _ = tx_res.send(DbResponseEvent::ShutdownComplete);
                break;
            }
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
                return Ok(EdgeFull {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    target: row.get(2)?,
                    edge_cluster_id: row.get(3)?,
                    temporal_props: temporal_props,
                });
            })
            .unwrap();

        let mut results: Vec<EdgeFull> = Vec::new();
        for r in rows {
            results.push(r.unwrap());
        }

        let _ = tx_res.send(DbResponseEvent::Edges(results));
    }
}


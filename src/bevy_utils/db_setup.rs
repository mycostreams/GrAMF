use crate::{
    bevy_utils::db_usage::{DbRequestEvent, DbResponseEvent},
    graph_model::{
        edges::{EdgeFull, EdgeTemporals},
        types::TimeSeries,
    },
};
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use rusqlite::{Connection, params_from_iter};

/// Generally used service linked to the database.
pub(crate) struct DbService {
    conn: Connection,
}

enum RequestHandleResult {
    Success,
    Shutdown,
}

/// Creation of the database, should only be called at the start of the app.
/// Continually runs during the app, consuming request events.
pub(super) fn run_db_service(rx_req: Receiver<DbRequestEvent>, tx_res: Sender<DbResponseEvent>) {
    println!("Starting Service");
    let mut service = DbService::new(&crate::io::db_io::open_default_path_db()).unwrap();

    println!("Created db service");

    // Here we continuously read messages, and process them.
    while let Ok(request) = rx_req.recv() {
        match handle_request(request, &mut service, &tx_res) {
            Ok(result_val) => match result_val {
                RequestHandleResult::Success => continue,
                RequestHandleResult::Shutdown => break,
            },
            //TODO: Change later to log and restart service
            Err(_) => break,
        }
    }
}

/// Middle-man function to select a method based on event type.
fn handle_request(
    request: DbRequestEvent,
    service: &mut DbService,
    tx_res: &Sender<DbResponseEvent>,
) -> Result<RequestHandleResult, anyhow::Error> {
    match request {
        DbRequestEvent::Startup => {
            tx_res.send(DbResponseEvent::Started).unwrap();
            Ok(RequestHandleResult::Success)
        }
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
        let new_service = Self { conn };
        new_service.init_edge_schema()?;
        Ok(new_service)
    }

    fn init_edge_schema(&self) -> Result<(), rusqlite::Error> {
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

        let edge_ids_sql_format = edge_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

        let sql_command = format!(
            "
            SELECT   edge_id, source, target, cluster_id, temporal_props
            FROM edge
            WHERE edge_id IN ({})
            ",
            edge_ids_sql_format
        );

        let mut stmt = self.conn.prepare(&sql_command)?;

        let rows = stmt.query_map(params_from_iter(edge_ids.iter()), |row| {
            parse_sql_edge_row(row)
        })?;

        let mut results: Vec<EdgeFull> = Vec::new();
        for r in rows {
            results.push(r?);
        }

        Ok(results)
    }
}

fn parse_sql_edge_row(row: &rusqlite::Row<'_>) -> Result<EdgeFull, rusqlite::Error> {
    let temporal_props_blob: Vec<u8> = row.get(4)?;
    let temporal_props: TimeSeries<EdgeTemporals> =
        postcard::from_bytes(&temporal_props_blob).unwrap();
    Ok(EdgeFull {
        id: row.get(0)?,
        source: row.get(1)?,
        target: row.get(2)?,
        edge_cluster_id: row.get(3)?,
        temporal_props,
    })
}

#[cfg(test)]
mod db_tests {
    use crate::{bevy_utils::db_setup::DbService, graph_model::edges::EdgeFull};

    impl super::DbService {
        fn from_memory() -> Result<Self, rusqlite::Error> {
            let conn = rusqlite::Connection::open_in_memory()?;
            let new_service = Self { conn };
            new_service.init_edge_schema()?;
            Ok(new_service)
        }
    }

    #[test]
    fn test_database_init() {
        // From memory to not interfere with integration test
        let mut service = DbService::from_memory().unwrap();

        let trial_data = vec![EdgeFull::new()];

        let return_size = service.insert_edges(trial_data).unwrap();

        assert!(return_size == 1);

        let queried_edges = service.query_edges(vec![1]).unwrap();

        println!("{:?}", queried_edges)
    }
}

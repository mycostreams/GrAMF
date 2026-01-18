use bevy::prelude::*;

use std::thread::JoinHandle;

use crate::bevy_utils::db_setup::run_db_service;
use crate::graph_model::edges::EdgeFull;
use crate::graph_model::nodes::NodeID;
use crossbeam_channel::unbounded;
use crossbeam_channel::{Receiver, Sender};

use bevy::app::AppExit;

pub struct DbStartupComplete;

/// Add db systems to the app. in order to add new edges you need to write a message to the system.
pub fn edge_db_plugin(app: &mut App) {
    app.add_message::<DbRequestEvent>()
        .add_message::<DbResponseEvent>()
        .add_systems(Startup, setup_database)
        .add_systems(Update, db_event_sender)
        .add_systems(Update, db_event_receiver)
        .add_systems(Update, db_shutdown);
}

/// Kinds of messages that can be sent to the database.
///
/// For now, we focus on inserting and querying edges and a shutdown.
/// TODO: Add function to add column to edges and fill with data (e.g. BC calculation for graph)
#[derive(Message, Clone, Debug)]
pub(crate) enum DbRequestEvent {
    Startup,
    InsertEdges(Vec<EdgeFull>),
    QueryEdges(Vec<NodeID>),
    Shutdown,
}

/// Responses that can be obtained from the db.
#[derive(Message, Debug)]
pub(crate) enum DbResponseEvent {
    Started,
    InsertProgress { inserted: usize },
    Edges(Vec<EdgeFull>),
    ShutdownComplete,
}

/// Bevy Resource for sending and interpreting db messages.
#[derive(Resource)]
pub(crate) struct DbWorker {
    pub(crate) tx: Sender<DbRequestEvent>,
    pub(crate) rx: Receiver<DbResponseEvent>,
    pub(crate) handle: Option<JoinHandle<()>>,
}

fn setup_database(mut commands: Commands) {
    // Set up sender and receiver for db requests
    // Sender goes to worker, receiver goes to service
    let (tx_req, rx_req) = unbounded();

    // Set up sender and receiver for db responses
    // Sender goes to db service, receiver goes to worker
    let (tx_res, rx_res) = unbounded();

    println!("Creating service");
    // Create separate thread to run db-related tasks
    // Service will respond to queries
    let handle = std::thread::spawn(move || {
        run_db_service(rx_req, tx_res);
    });

    tx_req.send(DbRequestEvent::Startup).unwrap();
    if let DbResponseEvent::Started = rx_res.recv().unwrap() {
    } else {
        panic!("Got weird message! From edge db")
    };

    // Insert db worker into Bevy engine
    // Worker will make queries and send inserts
    commands.insert_resource(DbWorker {
        tx: tx_req,
        rx: rx_res,
        handle: Some(handle),
    });
}

/// Read from messages, and send them to the db.
fn db_event_sender(db: Res<DbWorker>, mut reader: MessageReader<DbRequestEvent>) {
    for ev in reader.read() {
        let _ = db.tx.send(ev.clone());
    }
}

/// Read events from db, write them as messages
fn db_event_receiver(db: Res<DbWorker>, mut writer: MessageWriter<DbResponseEvent>) {
    while let Ok(ev) = db.rx.try_recv() {
        writer.write(ev);
    }
}

/// Shut down db upon getting an appexit message
fn db_shutdown(mut exit: MessageReader<AppExit>, mut db: ResMut<DbWorker>) {
    if exit.read().next().is_some() {
        let _ = db.tx.send(DbRequestEvent::Shutdown);

        if let Some(handle) = db.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Integration test, so also opens a folder on the computer on disc.
/// Other tests should init db from memory
#[test]
fn test_db_system() {
    let mut app = App::new();

    app.add_plugins(edge_db_plugin);

    app.world_mut()
        .resource_mut::<Messages<DbRequestEvent>>()
        .write(DbRequestEvent::InsertEdges(vec![EdgeFull::new()]));

    app.update();

    app.world_mut()
        .resource_mut::<Messages<DbRequestEvent>>()
        .write(DbRequestEvent::QueryEdges(vec![1]));

    app.update();

    let db_res_messages = app.world().resource::<Messages<DbResponseEvent>>();

    println!("{}", db_res_messages.len());

    let mut enemy_died_cursor = db_res_messages.get_cursor();
    let enemy_died = enemy_died_cursor.read(db_res_messages).next().unwrap();
    println!("{:?}", enemy_died);
}

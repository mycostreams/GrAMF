use bevy::prelude::*;

use std::thread::JoinHandle;

use crate::bevy_utils::db_setup::run_db_service;
use crate::graph_model::edges::EdgeFull;
use crossbeam_channel::unbounded;
use crossbeam_channel::{Receiver, Sender};

pub fn edge_db_plugin(app: &mut App) {
    app.add_message::<DbRequestEvent>()
        .add_message::<DbResponseEvent>()
        .add_systems(Startup, setup_database)
        .add_systems(Update, db_event_sender)
        .add_systems(Update, db_event_receiver)
        .add_systems(Update, db_shutdown);
}

fn setup_database(mut commands: Commands) {
    let (tx_req, rx_req) = unbounded();
    let (tx_res, rx_res) = unbounded();

    let handle = std::thread::spawn(move || {
        run_db_service(rx_req, tx_res);
    });

    commands.insert_resource(DbWorker {
        tx: tx_req,
        rx: rx_res,
        handle: Some(handle),
    });
}

#[derive(Message, Clone, Debug)]
pub enum DbRequestEvent {
    InsertEdges(Vec<EdgeFull>),
    QueryEdges(Vec<i64>),
    Shutdown,
}

#[derive(Message, Debug)]
pub enum DbResponseEvent {
    InsertProgress { inserted: usize },
    Edges(Vec<EdgeFull>),
    ShutdownComplete,
}

#[derive(Resource)]
pub struct DbWorker {
    pub tx: Sender<DbRequestEvent>,
    pub rx: Receiver<DbResponseEvent>,
    pub handle: Option<JoinHandle<()>>,
}

fn db_event_sender(db: Res<DbWorker>, mut reader: MessageReader<DbRequestEvent>) {
    for ev in reader.read() {
        let _ = db.tx.send(ev.clone());
    }
}

fn db_event_receiver(db: Res<DbWorker>, mut writer: MessageWriter<DbResponseEvent>) {
    while let Ok(ev) = db.rx.try_recv() {
        writer.write(ev);
    }
}

use bevy::app::AppExit;

fn db_shutdown(mut exit: MessageReader<AppExit>, mut db: ResMut<DbWorker>) {
    if exit.read().next().is_some() {
        let _ = db.tx.send(DbRequestEvent::Shutdown);

        if let Some(handle) = db.handle.take() {
            let _ = handle.join();
        }
    }
}

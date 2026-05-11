// app/context.rs

use super::messages::AppCommand;

pub struct AppContext {
    pub commands: Vec<AppCommand>,
}

impl AppContext {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn enqueue(&mut self, cmd: AppCommand) {
        self.commands.push(cmd);
    }
}

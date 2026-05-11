// app/lifecycle.rs

use super::*;

pub fn process_commands(_app: &AppState, ctx: &mut AppContext) {
    for cmd in ctx.commands.drain(..) {
        match cmd {
            AppCommand::Redraw => {
                // stub — in later phases this calls renderer
                println!("Redraw requested");
            }
        }
    }
}
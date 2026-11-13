use octapus_core::models::ServerGroup;
use octapus_core::storage::repositories::server_groups;
use serde_json::{json, Value};

use crate::cli::GroupCommand;
use crate::context::App;
use crate::error::CliResult;
use crate::output::Grid;

fn value(group: &ServerGroup) -> Value {
    serde_json::to_value(group).unwrap_or(Value::Null)
}

pub fn run(app: &App, command: GroupCommand) -> CliResult<()> {
    let storage = &app.state.storage;
    match command {
        GroupCommand::List => {
            let groups = server_groups::get_all(storage)?;
            let mut grid = Grid::new(["ID", "NOME"]);
            for group in &groups {
                grid.push([Some(group.id.to_string()), Some(group.name.clone())]);
            }
            app.out.list(&grid, &groups.iter().map(value).collect::<Vec<_>>())
        }
        GroupCommand::Add { name } => {
            let group = server_groups::create(storage, &name)?;
            app.out.done(&format!("grupo {} criado (id {})", group.name, group.id), json!({ "group": value(&group) }))
        }
        GroupCommand::Rename { group, name } => {
            let current = app.group(&group)?;
            let renamed = server_groups::rename(storage, current.id, &name)?;
            app.out.done(&format!("grupo renomeado para {}", renamed.name), json!({ "group": value(&renamed) }))
        }
        GroupCommand::Rm { group } => {
            let current = app.group(&group)?;
            server_groups::delete(storage, current.id)?;
            app.out.done(&format!("grupo {} excluído", current.name), json!({ "id": current.id }))
        }
    }
}

use valence::{
    app::{App, Plugin},
    command::{
        AddCommand, CommandScopeRegistry, handler::CommandResultEvent, parsers::{EntitySelector, entity_selector::EntitySelectors}
    },
    command_macros::Command,
    message::ChatMessageEvent,
    prelude::*,
};

use crate::{
    plugins::doom::{DoomSessionDirectory, DoomSessionRegistry},
    utils::inventory::InventoryExt,
};

/// Plugin that adds chat functionality to the server.
/// Players can send and receive messages in the chat.
pub struct DoomCommandPlugin;

impl Plugin for DoomCommandPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .add_command::<ViewCommand>()
        .add_systems(Startup, setup_commands)
        .add_systems(Update, 
            handle_view_commands
        );
    }
}

#[derive(Command, Debug, Clone)]
#[paths("view")]
#[scopes("valence.command.view")]
enum ViewCommand {
    #[paths = "{target}"]
    Target{ target: EntitySelector },
}

fn handle_view_commands(
    d_dir: Res<DoomSessionDirectory>,
    mut d_reg: ResMut<DoomSessionRegistry>,
    mut events: EventReader<CommandResultEvent<ViewCommand>>,
    mut clients: Query<(&mut Client, &mut Inventory)>,
    usernames: Query<(Entity, &Username)>,
) {
    for event in events.read() {
        let Ok((sender_client, inventory)) = &mut clients.get_mut(event.executor) else {
            continue;
        };

        let target_entity = match &event.result {
                ViewCommand::Target { target } => { match target {
                    EntitySelector::SimpleSelector(EntitySelectors::SinglePlayer(name)) => usernames
                    .iter()
                    .find(|(_, username)| username.0 == *name)
                    .map(|(entity, _)| entity),
                    _ => return,
                }
            }
        };

        let Some(target_entity) = target_entity else {
            sender_client.send_chat_message("Could not find target".to_string());
            return;
        };

        // Resolve entity to session id
        let Some(session_id) = d_dir.id_of(target_entity) else {
            sender_client.send_chat_message("Target has no DOOM session.".to_string());
            continue;
        };

        // Bind map
        if let Some(map) = d_reg.create_map_view(session_id) {
            let _ = inventory.swap_first(|stack| stack.item == ItemKind::FilledMap, map);
            sender_client.send_chat_message("Viewing target.".to_string());
        } else {
            sender_client.send_chat_message("Failed to create view map.");
        }
    }
}

fn setup_commands(
    mut command_scopes: ResMut<CommandScopeRegistry>,
) {
    command_scopes.link("valence.admin", "valence.command");
}
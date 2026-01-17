use valence::command::parsers::entity_selector::EntitySelectors;
use valence::command::parsers::EntitySelector;
use valence::message::SendMessage;
use valence::{
    app::{App, Plugin},
    command::{
        AddCommand, CommandScopeRegistry, handler::CommandResultEvent,
    },
    command_macros::Command,
    prelude::*,
};

use crate::{
    plugins::doom::{DoomSessionDirectory, DoomSessionRegistry},
    extensions::inventory::InventoryExt,
};

macro_rules! require_single_player_entity {
    ($selector:expr, $usernames:expr, $sender:expr, $invalid:expr) => {{
        let EntitySelector::SimpleSelector(EntitySelectors::SinglePlayer(name)) = $selector else {
            $sender.send_chat_message($invalid);
            return;
        };

        match $usernames
            .iter()
            .find(|(_, u)| u.0 == *name)
            .map(|(e, _)| e)
        {
            Some(entity) => (entity, name),
            None => {
                $sender.send_chat_message(format!("Could not find player: {name}"));
                return;
            }
        }
    }};
}

/// Plugin that adds chat functionality to the server.
/// Players can send and receive messages in the chat.
pub struct DoomCommandPlugin;

impl Plugin for DoomCommandPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .add_command::<WatchCommand>()
        .add_command::<UnwatchCommand>()
        .add_systems(Startup, setup_commands)
        .add_systems(Update, 
            (
                handle_watch_commands,
                handle_unwatch_commands
            )
        );
    }
}


#[derive(Command, Debug, Clone)]
#[paths("watch {target}")]
#[scopes("doom.command.watch")]
struct WatchCommand {
    target: EntitySelector,
}

#[derive(Command, Debug, Clone)]
#[paths("unwatch")]
#[scopes("doom.command.unwatch")]
struct UnwatchCommand;

fn handle_watch_commands(
    d_dir: Res<DoomSessionDirectory>,
    mut d_reg: ResMut<DoomSessionRegistry>,
    mut events: EventReader<CommandResultEvent<WatchCommand>>,
    mut clients: Query<(&mut Client, &mut Inventory)>,
    usernames: Query<(Entity, &Username)>,
) {
    for event in events.read() {
        let Ok((sender_client, inventory)) = &mut clients.get_mut(event.executor) else {
            continue;
        };

        let (target_entity, target_name) = require_single_player_entity!(
            &event.result.target,
            usernames,
            sender_client,
            "Usage: /view <player>".to_string()
        );

        // Resolve entity to session id
        let Some(session_id) = d_dir.id_of(target_entity) else {
            sender_client.send_chat_message(format!("{target_name} has no DOOM session."));
            continue;
        };

        // Bind map
        if let Some(map) = d_reg.create_map_view(session_id) {
            let _ = inventory.swap_first(|stack| stack.item == ItemKind::FilledMap, map);
            sender_client.send_chat_message(format!("Viewing session of {target_name}"));
        } else {
            sender_client.send_chat_message(format!("Failed to create view map for {target_name}."));
        }
    }
}

fn handle_unwatch_commands(
    d_dir: Res<DoomSessionDirectory>,
    mut d_reg: ResMut<DoomSessionRegistry>,
    mut events: EventReader<CommandResultEvent<UnwatchCommand>>,
    mut clients: Query<(&mut Client, &mut Inventory)>,
) {
    for event in events.read() {
        let Ok((sender_client, inventory)) = &mut clients.get_mut(event.executor) else {
            continue;
        };

        // Resolve entity to session id
        let Some(session_id) = d_dir.id_of(event.executor) else {
            sender_client.send_chat_message("You do not have a doom session");
            continue;
        };

        // Bind map
        if let Some(map) = d_reg.create_map_view(session_id) {
            let _ = inventory.swap_first(|stack| stack.item == ItemKind::FilledMap, map);
        }
    }
}

fn setup_commands(
    mut command_scopes: ResMut<CommandScopeRegistry>,
) {
    command_scopes.link("doom.admin", "doom.command");
}


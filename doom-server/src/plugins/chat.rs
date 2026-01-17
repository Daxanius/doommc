use valence::{
    app::{App, Plugin},
    message::ChatMessageEvent,
    prelude::*,
};

/// Plugin that adds chat functionality to the server.
/// Players can send and receive messages in the chat.
pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .insert_resource(ChatHistory::default())
        .add_systems(Update, on_chat_message);
    }
}

#[derive(Resource, Default)]
pub struct ChatHistory {
    history: Vec<String>,
    capacity: usize,
}

impl ChatHistory {
    pub fn push(&mut self, text: String) {
        self.history.push(text);
        if self.history.len() > self.capacity {
            self.history.pop();
        }
    }

    #[must_use]
    pub fn get_history(&self) -> &Vec<String> {
        &self.history
    }
}

#[allow(clippy::type_complexity)]
fn on_player_join(mut p: ParamSet<(Query<&Username, Added<Client>>, Query<&mut Client>)>) {
    let joined_names: Vec<String> = p
        .p0()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    for name in joined_names {
        for mut client in &mut p.p1() {
            client.send_chat_message(
                format!("{name} has joined to RIP AND TEAR")
                    .into_text()
                    .color(Color::RED),
            );
        }
    }
}

fn on_chat_message(
    mut chat: ResMut<ChatHistory>,
    mut ev: EventReader<ChatMessageEvent>,
    mut clients: Query<&mut Client>,
    username: Query<&Username>,
) {
    for ev in ev.read() {
        chat.push(ev.message.clone().into_string());
        let Ok(username) = username.get(ev.client) else {
            return;
        };

        for mut client in &mut clients {
            client.send_chat_message(format!(
                "{}: {}",
                username,
                ev.message.clone().into_string()
            ));
        }
    }
}

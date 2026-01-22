use ipc_channel::ipc::IpcSender;
use serde::{Deserialize, Serialize};

use crate::packet::{Frame, Input, TicCmd};

pub mod packet;
pub mod palette;
pub mod util;

pub const MAP_WIDTH: usize = 128;
pub const MAP_HEIGHT: usize = 128;

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerCommand {
    Input {
        input: Input,
        pressed: bool,
    },
    State {
        active: bool,
    },
    RotationDelta(i16),

    RegisterPipes {
        frame_tx: IpcSender<Frame>,
        event_tx: IpcSender<ClientEvent>,
    },
    NetCmdBundle(Vec<TicCmd>),
    NetJoin {
        id: i32,
    },
    NetLeave {
        id: i32,
    },
}

#[derive(Serialize, Deserialize, Debug)]
// #[allow(clippy::large_enum_variant)] // Lol, the frame makes this enum huge
pub enum ClientEvent {
    TicCmd(TicCmd),
}

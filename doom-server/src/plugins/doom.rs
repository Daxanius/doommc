use valence::prelude::*;

use crate::plugins::doom::{
    command::DoomCommandPlugin, input::DoomInputPlugin, net::DoomNetPlugin,
    session::DoomSessionPlugin, video::DoomVideoPlugin,
};

pub mod command;
pub mod input;
pub mod net;
pub mod session;
pub mod util;
pub mod video;

/// Doom plugin bundle providing all the plugins necessary for a full
/// playing experience.
pub struct DoomPluginBundle;

impl Plugin for DoomPluginBundle {
    fn build(&self, app: &mut App) {
        #[rustfmt::skip]
        app
        .add_plugins((
            DoomSessionPlugin,
            DoomVideoPlugin,
            DoomInputPlugin,
            DoomNetPlugin,
            DoomCommandPlugin
        ));
    }
}

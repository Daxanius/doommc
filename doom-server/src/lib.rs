pub mod extensions;
pub mod plugins;

pub const TICK_RATE: u32 = 35;

include!(concat!(env!("OUT_DIR"), "/resource_pack_consts.rs"));

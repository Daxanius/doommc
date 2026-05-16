// Maps DOOMs PLAYPAL color palette to Minecrafts map palette.
// https://doom.fandom.com/wiki/PLAYPAL
// https://minecraft.fandom.com/wiki/Map_item_format
// #[rustfmt::skip]
// const PALETTE_MAPPING: [u8; 256] = [
//     29, 47, 43, 59, 8, 21, 21, 29, 29, 27, 41, 49, 51, 13, 10, 26,
//     60, 60, 36, 36, 36, 36, 36, 42, 42, 42, 42, 42, 50, 50, 52, 52,
//     28, 28, 28, 28, 28, 28, 28, 54, 54, 54, 54, 54, 36, 36, 36, 36,
//     8, 14, 14, 60, 60, 60, 60, 36, 36, 36, 36, 44, 44, 44, 44, 44,
// ];

include!(concat!(env!("OUT_DIR"), "/palette_mapping.rs"));

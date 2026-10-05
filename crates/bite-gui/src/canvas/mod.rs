//! The node canvas: a Rust-owned surface drawn through ImGui draw lists.
//!
//! Dear ImGui's node editor add-on owns its own selection, navigation and styling, which
//! does not match the Svelte Flow behavior the Electron editor exposes. The canvas here
//! reproduces that behavior directly instead.
pub mod layout;
pub mod state;
pub mod view;
pub mod viewport;
pub mod wire;

//! bwr-core: physics engine for a GE Hitachi BWRX-300 control room simulator.
//!
//! The engine is UI-agnostic: create a [`sim::Sim`], feed it [`api::Command`]s,
//! call [`sim::Sim::advance`] and read [`api::Snapshot`]s.

pub mod api;
pub mod bop;
pub mod ic;
pub mod kinetics;
pub mod params;
pub mod reactor;
pub mod sim;
pub mod steam;
pub mod steam_table;
pub mod vessel;

pub use api::{Command, Snapshot};
pub use sim::Sim;

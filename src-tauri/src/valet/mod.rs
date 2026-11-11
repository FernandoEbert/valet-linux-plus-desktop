//! Thin layer over the valet-linux-plus CLI. No business rules live here: reading goes through
//! `valet state --json` (or a read-only mirror of `~/.config/valet` for older versions).

pub mod cli;
pub mod env;
pub mod error;
pub mod files;
pub mod locate;
pub mod model;
pub mod runner;
pub mod services;
pub mod state;

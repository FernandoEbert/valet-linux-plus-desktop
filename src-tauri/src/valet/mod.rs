//! Thin layer over the valet-linux-plus CLI. No business rules live here: reading goes through
//! `valet state --json` (or a read-only mirror of `~/.config/valet` for older versions) and every
//! mutation is a whitelisted `valet <command>` call.

pub mod actions;
pub mod cli;
pub mod env;
pub mod error;
pub mod files;
pub mod locate;
pub mod logs;
pub mod model;
pub mod privileged;
pub mod runner;
pub mod services;
pub mod state;

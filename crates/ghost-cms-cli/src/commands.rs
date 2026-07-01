//! Subcommand implementations. Each is a `clap::Args` struct that implements
//! [`crate::command::Command`].

pub(crate) mod delete;
pub(crate) mod doctor;
pub(crate) mod edit;
pub(crate) mod get;
pub(crate) mod init;
pub(crate) mod list;
pub(crate) mod meta;
pub(crate) mod new;
pub(crate) mod open;
pub(crate) mod publish;
pub(crate) mod tags;
pub(crate) mod upload;
pub(crate) mod watch;
pub(crate) mod whoami;

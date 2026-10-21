//! Native (Linux/Windows) side of the launcher: finds the game and Proton,
//! manages profiles, and starts sc-inject.exe. Kept as a library so a GUI can
//! wrap it later.

#[cfg(unix)]
pub mod compat;
pub mod env;
pub mod launch;
pub mod packs;
pub mod patching;
pub mod profiles;
pub mod steam;

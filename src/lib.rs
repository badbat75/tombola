// lib.rs
// Library modules for tombola game

pub mod defs;
pub mod pouch;
pub mod board;
pub mod server;
pub mod card;
pub mod client;
pub mod score;
pub mod extraction;
pub mod config;
pub mod logging;
pub mod game;
pub mod api_handlers;

// Client library modules
pub mod clients {
    pub mod common;
    pub mod game_utils;
    pub mod api_client;
    pub mod card_management;
    pub mod registration;
    pub mod terminal;
}

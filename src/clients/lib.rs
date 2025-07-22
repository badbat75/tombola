// src/clients/lib.rs
// Client library for the Tombola game
//
// This library provides shared functionality for tombola client applications
// including common data structures, HTTP utilities, and API communication.
//
// The modular design eliminates code duplication between clients while
// maintaining clean separation of concerns. Each module serves a specific purpose:
//
// - common: Shared data structures and HTTP utilities
// - game_utils: Game discovery and management utilities
// - api_client: Centralized HTTP API client functions
// - card_management: Card-specific operations
// - registration: Client registration and authentication
// - terminal: Terminal UI utilities for board display
//
// This architecture allows each client to use only the modules it needs,
// creating a flexible and maintainable codebase.

pub mod terminal;
pub mod common;
pub mod game_utils;
pub mod api_client;
pub mod card_management;
pub mod registration;

# Tombola Game

A Rust-based multi-binary tombola (bingo) game with a client-server architecture and HTTP API.

## Architecture

This project consists of three main binaries:

- **`tombola-server`**: Main game server with terminal UI and HTTP API
- **`board_client`**: Terminal client that displays current game state  
- **`card_client`**: Interactive client for card management and gameplay

## Features

- **Server Components:**
  - Visual board display with proper spacing and color coding
  - Real-time score checking (2, 3, 4, 5 in a row, BINGO)
  - HTTP API for client integration
  - Thread-safe shared state management
  - Card generation with anti-adjacency patterns

- **Client Components:**
  - Terminal-based board display client
  - Interactive card management client
  - HTTP API integration with authentication

## Configuration

The game uses configurable card layouts:
- Default: 2×3 grid of cards (6 cards total)
- Each card contains 5×3 numbers (15 numbers per card)
- Numbers range from 1-90
- Cards follow tombola rules with proper column distribution

## Build and Run

```bash
# Build all binaries
cargo build --release

# Run main server (includes terminal UI and HTTP API)
cargo run --bin tombola-server

# Run display-only client
cargo run --bin board_client

# Run interactive card client  
cargo run --bin card_client
```

## HTTP API

The server provides a RESTful HTTP API on `http://127.0.0.1:3000`. See `docs/TOMBOLA_API.md` for complete API documentation.

### Key Endpoints:
- `POST /register` - Register a new client
- `GET /board` - Get current extracted numbers
- `GET /pouch` - Get remaining numbers  
- `GET /scoremap` - Get current scores and winners
- `POST /generatecardsforme` - Generate cards for a client
- `GET /listassignedcards` - List assigned cards

## Server Controls

- **Any key**: Draw next number from pouch
- **ESC**: Exit server

## Dependencies

- `crossterm` - For terminal manipulation and keyboard input
- `rand` - For random number generation
- `tokio` - Async runtime
- `hyper` - HTTP server
- `reqwest` - HTTP client (for client binaries)
- `serde` - JSON serialization

## Development

See `.github/copilot-instructions.md` for detailed development guidelines and architectural patterns.
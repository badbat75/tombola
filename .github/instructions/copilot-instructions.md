# Tombola Game - AI Coding Assistant Instructions

## Project Information
- **Version**: 0.8.0
- **Edition**: 2024
- **Language**: Rust
- **License**: GPLv3.0

## Architecture Overview

This is a Rust-based multi-binary tombola/bingo game with a client-server architecture:

- **`tombola-server`** (`src/tombola_server.rs`): Main game server with terminal UI and HTTP API
- **`tombola-client`** (`src/tombola_client.rs`): Terminal client that displays current game state with interactive controls
- **`tombola-player`** (`src/card_client.rs`): Interactive client for card management and gameplay

## Core Components & Data Flow

### New Modular Architecture
Recent updates introduce improved modularity with additional components:

- **`config.rs`**: Configuration management for server/client settings with file-based configuration support
- **`logging.rs`**: Centralized logging utility with timestamp formatting and log levels
- **`extraction.rs`**: Core extraction logic shared between server and API components
- **`lib.rs`**: Library structure for shared functionality across binaries

### Shared State Management
The server uses a unified **Game super struct** that encapsulates all game state:
- **Game struct** (`src/game.rs`): Unified game state management with unique IDs and timestamps
- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal ID (format: `game_12345678`)
- **Creation Timestamps**: Games include creation timestamps with human-readable formatting
- **Thread-Safe Components**: Board, Pouch, ScoreCard, CardAssignmentManager, ClientRegistry
- **Coordinated Access**: All components wrapped in `Arc<Mutex<T>>` for thread safety
- **Enhanced API**: Game ID and creation time included in status and reset responses

### Critical Mutex Coordination Pattern
**Always use the Game struct methods** for coordinated access to prevent deadlocks:
```rust
// CORRECT: Use Game struct methods for coordinated operations
let game = Game::new();
if let Ok(success) = game.reset_game() {
    // Game reset with proper mutex coordination
}

// Access individual components through Game methods:
let board_len = game.board_length();
let scorecard = game.published_score();
```

**Legacy pattern (avoid for new code)**: Direct mutex access should be replaced with Game methods:
```rust
// LEGACY: Direct mutex coordination (now encapsulated in Game struct)
if let Ok(pouch) = pouch_ref.lock() {
    if let Ok(board) = board_ref.lock() {
        if let Ok(scorecard) = scorecard_ref.lock() {
            // Perform coordinated operations
        }
    }
}
```

### Game Configuration (`src/defs.rs`)
- `BOARDCONFIG`: Defines card layout (5×3 numbers, 2×3 card grid)
- `NUMBERSPERCARD`: 15 numbers per card (5 columns × 3 rows)
- Numbers range: 1-90 (calculated as `FIRSTNUMBER` to `LASTNUMBER`)
- `Colors`: Terminal color definitions for UI formatting (Green, Yellow, Red, Blue, Magenta)

### Configuration Management (`src/config.rs`)
- `ServerConfig`: Host/port configuration with defaults (127.0.0.1:3000)
- `ClientConfig`: Client connection settings including timeouts and retry logic
- File-based configuration support for both server and client settings
- Default configuration fallback when files are missing

### Logging System (`src/logging.rs`)
- Centralized logging with `LogLevel` enum (Info, Error, Warning)
- Automatic timestamp formatting using chrono
- Consistent log message formatting across all components
- Thread-safe logging utilities for server operations

### Extraction Engine (`src/extraction.rs`)
- Core extraction logic separated from UI components
- Shared between terminal server and HTTP API
- Handles mutex coordination for thread-safe operations
- Validates game state before performing extractions

## Key Development Patterns

### HTTP API Server (`src/server.rs`)
- Hyper-based async server on `127.0.0.1:3000`
- All endpoints return JSON with CORS headers
- Client authentication via `X-Client-ID` header
- Error responses use standard HTTP status codes
- Client registration restricted to pre-game state (before any number extraction)

### Client Registry (`src/client.rs`)
- Thread-safe client registration and management
- Prevents new client registration after game starts (numbers extracted)
- Uses board state to enforce registration timing restrictions

### Card Generation Algorithm (`src/card.rs`)
Cards are generated as groups of 6 with anti-adjacency rules:
- Each card has 15 numbers distributed across 9 columns (1-10, 11-20, ..., 81-90)
- Numbers are positioned to avoid adjacent placement across cards
- Use `CardManagement::generate_card_group()` for compliant card sets

### Terminal UI (`src/terminal.rs`)
- Uses crossterm for cross-platform terminal control
- Color coding: Green for current number, Yellow for marked/winning numbers
- Board layout calculated with `downrightshift()` for proper spacing
- Interactive controls for clients:
  - `tombola-client`: ENTER to extract, F5 to refresh, ESC to exit
  - `tombola-server`: Any key to extract, ESC to exit
  - CLI support for game reset with `--newgame` option

## Build & Run Commands

```bash
# Build all binaries
cargo build --release

# Run main server (includes terminal UI)
cargo run --bin tombola-server

# Run display-only client
cargo run --bin tombola-client

# Run display-only client with game reset
cargo run --bin tombola-client -- --newgame

# Run interactive card client
cargo run --bin tombola-player
```

## API Integration Patterns

### Client Registration Flow
1. POST `/register` with `{name, client_type, nocard}`
2. Store returned `client_id` for subsequent requests
3. Use `X-Client-ID` header for authenticated endpoints

**Important Registration Rule**: New clients can only register when no numbers have been extracted from the pouch. Once the first number is extracted, registration attempts will fail with a 409 Conflict error. This ensures fair play by preventing mid-game registration.

### Real-time Game State
- GET `/board` - Current extracted numbers
- GET `/pouch` - Available numbers in pouch
- GET `/scoremap` - Current scores and winners with score map

### Card Management
- POST `/generatecards` - Generate new card sets
- POST `/assigncard` - Assign cards to clients
- GET `/listassignedcards` - View all assignments

## Testing & Debugging

- Server logs to stdout with connection and error details using the centralized logging system
- Use `docs/TOMBOLA_API.md` for complete API reference
- Test API endpoints with curl using examples in documentation
- Terminal clients provide immediate visual feedback for server state
- Client supports CLI options like `--newgame` for game reset functionality

## Dependencies

Current project dependencies (Cargo.toml):
- `rand` - Random number generation for pouch extraction
- `crossterm` - Cross-platform terminal manipulation and keyboard input
- `tokio` - Async runtime with macros, rt-multi-thread, net, and time features
- `reqwest` - HTTP client with JSON support (for client binaries)
- `serde` - Serialization framework with derive features
- `serde_json` - JSON serialization support
- `hyper` - HTTP server with server and http1 features
- `hyper-util` - Hyper utilities with tokio features
- `http-body-util` - HTTP body utilities
- `chrono` - Date and time library for logging timestamps
- `clap` - Command line argument parsing with derive features

## File Organization

- `src/game.rs`: Unified game state management with unique IDs and timestamps
- `src/defs.rs`: Core constants and type definitions
- `src/board.rs`: Game board state management
- `src/score.rs`: Scoring logic and prize calculations
- `src/card.rs`: Card generation and assignment logic
- `src/client.rs`: Client registration and management
- `src/server.rs`: HTTP API server implementation
- `src/terminal.rs`: Terminal UI rendering
- `src/pouch.rs`: Number extraction logic
- `src/config.rs`: Configuration management for server and client settings
- `src/logging.rs`: Centralized logging utilities
- `src/extraction.rs`: Shared extraction logic for server and API
- `src/lib.rs`: Library structure for shared functionality

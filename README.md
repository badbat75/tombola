# Tombola Game

A Rust-based multi-binary tombola (bingo) game with a client-server architecture and HTTP API.

## Architecture

This project consists of three main binaries:

- **`tombola-server`**: Main game server with terminal UI and HTTP API
- **`tombola-client`**: Terminal client that displays current game state  
- **`tombola-player`**: Interactive client for card management and gameplay

## Features

- **Server Components:**
  - Visual board display with proper spacing and color coding
  - Real-time score checking (2, 3, 4, 5 in a row, BINGO)
  - HTTP API for client integration
  - Unified Game state management with unique IDs and timestamps
  - Thread-safe shared state management with Arc<Mutex<T>>
  - Card generation with anti-adjacency patterns
  - Game reset functionality with complete state cleanup
  - Centralized logging system with timestamps
  - Modular extraction engine shared between terminal and API

- **Client Components:**
  - Terminal-based board display client with CLI options
  - Interactive card management client
  - HTTP API integration with authentication
  - Command-line game reset capabilities
  - File-based configuration support
  - Real-time game state synchronization

## Configuration

The game uses configurable card layouts and settings:
- Default: 2×3 grid of cards (6 cards total)
- Each card contains 5×3 numbers (15 numbers per card)
- Numbers range from 1-90 (calculated as FIRSTNUMBER to LASTNUMBER)
- Cards follow tombola rules with proper column distribution
- Server configuration: Host/port settings (default: 127.0.0.1:3000)
- Client configuration: Connection settings with timeouts and retry logic
- File-based configuration support with fallback to defaults

## Build and Run

```bash
# Build all binaries
cargo build --release

# Run main server (includes terminal UI and HTTP API)
cargo run --bin tombola-server

# Run display-only client
cargo run --bin tombola-client

# Run display-only client with game reset
cargo run --bin tombola-client -- --newgame

# Run interactive card client  
cargo run --bin tombola-player
```

## Client Options

### Board Client CLI Options

The `tombola-client` supports the following command-line options:

- `--newgame`: Reset the game state before starting the client interface
- `--help`: Display help information
- `--version`: Display version information

**Examples:**
```bash
# Start board client normally
cargo run --bin tombola-client

# Start board client with game reset
cargo run --bin tombola-client -- --newgame

# Get help information
cargo run --bin tombola-client -- --help
```

**Notes about --newgame option:**
- Only the board client can reset the game (uses client ID "0000000000000000")
- Resets all game components: Board, Pouch, ScoreCard, and CardAssignmentManager
- Displays confirmation of reset components before starting the normal client interface
- If the reset fails, the client continues with the current game state
- Equivalent to calling the `/newgame` API endpoint manually

## HTTP API

The server provides a RESTful HTTP API on `http://127.0.0.1:3000`. See `docs/TOMBOLA_API.md` for complete API documentation.

### Key Features:
- Client registration and authentication
- Card generation and assignment
- Number extraction with authorization controls
- Game state management and reset functionality
- Real-time score tracking

## Server Controls

- **Any key**: Draw next number from pouch
- **ESC**: Exit server

## Client Controls

### Board Client:
- **ENTER**: Extract a number from the pouch (when prompted)
- **F5**: Refresh the screen and update game state without extracting
- **ESC**: Exit the client

### Card Client:
- Interactive menu-driven interface for card management
- Card assignment and viewing capabilities
- Integration with HTTP API for real-time updates

## Core Architecture

### Game State Management:
- **Game Super Struct**: Unified `Game` struct that encapsulates all game state components
- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal ID (format: `game_12345678`)
- **Creation Timestamps**: Games include creation timestamps with human-readable formatting
- **Enhanced API Responses**: Game ID and creation time included in status and reset endpoints

### Modular Components:
- **`game.rs`**: Unified game state management with ID and timestamp tracking
- **`config.rs`**: Configuration management with file-based settings
- **`logging.rs`**: Centralized logging with timestamp formatting
- **`extraction.rs`**: Shared extraction logic between server and API
- **`lib.rs`**: Library structure for shared functionality

### Thread-Safe State Management:
- Uses `Arc<Mutex<T>>` for coordinated access to shared game state
- Consistent mutex acquisition order to prevent deadlocks
- Shared state includes: Board, Pouch, ScoreCard, CardAssignmentManager, ClientRegistry
- Unified through the Game struct with proper coordination methods

## Dependencies

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

## Development

See `.github/copilot-instructions.md` for detailed development guidelines and architectural patterns.
# Tombola Game

A Rust-based multi-binary tombola (bingo) game with a client-server architecture, multi-game support, and comprehensive HTTP API using Axum web framework.

## Architecture

This project consists of three main binaries:

- **`tombola-server`**: Main game server with terminal UI and HTTP API
- **`tombola-client`**: Terminal client that displays current game state
- **`tombola-player`**: Interactive client for card management and gameplay

### Multi-Game Architecture

The server now supports multiple concurrent games through a **GameRegistry** system:
- **Game-Specific API Routing**: All API endpoints use `/{game_id}/` routing for game isolation
- **Client Registration Per Game**: Clients register to specific games using `/{game_id}/register`
- **Independent Game States**: Each game maintains separate Board, Pouch, ScoreCard, and Client registries
- **Game Management**: Create new games via `/newgame` endpoint and list all games via `/gameslist`

## Features

- **Server Components:**
  - Visual board display with proper spacing and color coding
  - Real-time score checking (2, 3, 4, 5 in a row, BINGO)
  - Axum-based HTTP API with game-specific routing (`/{game_id}/endpoint`)
  - Multi-game support with GameRegistry for concurrent games
  - Game management endpoints (`/newgame`, `/gameslist`, `/{game_id}/dumpgame`)
  - Unified Game state management with unique IDs and timestamps
  - Thread-safe shared state management with Arc<Mutex<T>>
  - Card generation with anti-adjacency patterns
  - Game reset functionality with complete state cleanup
  - Centralized logging system with timestamps
  - Modular extraction engine shared between terminal and API
  - CORS support for web client integration

- **Client Components:**
  - Terminal-based board display client with CLI options
  - Interactive card management client with multi-game support
  - HTTP API integration with authentication via `X-Client-ID` headers
  - Command-line game reset capabilities through `/newgame` endpoint
  - File-based configuration support
  - Real-time game state synchronization across multiple games
  - Game selection and listing capabilities via `/gameslist` endpoint
  - Automatic game ID detection and management
  - **Smart Game Discovery**: Clients without specified game ID automatically list available games

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

# Run display-only client (shows available games and instructions if no game ID specified)
cargo run --bin tombola-client

# Run display-only client with specific game ID
cargo run --bin tombola-client -- --gameid game_12345678

# Run display-only client with new game creation
cargo run --bin tombola-client -- --newgame

# Run display-only client in non-interactive mode (list games and exit)
cargo run --bin tombola-client -- --exit

# Run interactive card client (shows available games and instructions if no game ID specified)
cargo run --bin tombola-player

# Run interactive card client with specific game and settings
cargo run --bin tombola-player -- --gameid game_12345678 --name "Player1" --nocard 3

# Run card client in non-interactive mode (list games/status and exit)
cargo run --bin tombola-player -- --exit
```

## Client Options

### Board Client CLI Options

The `tombola-client` supports the following command-line options:

- `--newgame`: Create a new game before starting the client interface
- `--gameid <GAME_ID>`: Specify the game ID to connect to
- `--exit`: Exit after displaying the current state (no interactive loop)
- `--listgames`: List available games and exit
- `--help`: Display help information
- `--version`: Display version information

**Default Behavior (No Game ID Specified):**
- Automatically calls `/gameslist` endpoint to display available games
- Shows game status, creation times, and statistics
- Exits with instructions to use `--gameid <id>` to join a specific game or `--newgame` to create one

**Examples:**
```bash
# Start board client normally (shows games list and instructions)
cargo run --bin tombola-client

# Start board client with new game creation
cargo run --bin tombola-client -- --newgame

# Connect to a specific game
cargo run --bin tombola-client -- --gameid game_12345678

# Display games list once and exit (non-interactive mode)
cargo run --bin tombola-client -- --exit

# Explicitly list games and exit
cargo run --bin tombola-client -- --listgames

# Combine options: create new game and exit after display
cargo run --bin tombola-client -- --newgame --exit

# Get help information
cargo run --bin tombola-client -- --help
```

**Notes about --newgame option:**
- Only the board client can create new games (uses client ID "0000000000000000")
- Creates a completely new game in the GameRegistry with a unique game ID
- The original game continues to exist and can be accessed separately
- New game is registered in the multi-game registry for independent access
- Displays confirmation with new game ID and creation timestamp
- If the creation fails, the client continues with the current game state
- Equivalent to calling the `/newgame` API endpoint manually
- **Multi-Game Behavior**: Does not reset existing games, but adds a new one to the registry

**Notes about --exit option:**
- Provides non-interactive mode for both board and player clients
- Displays current game state once and exits immediately
- Useful for automation, scripting, or status checking
- Can be combined with other options like --newgame

### Player Client CLI Options

The `tombola-player` supports the following command-line options:

- `--name <NAME>`: Set client name (overrides config file)
- `--gameid <GAME_ID>`: Specify the game ID to connect to
- `--nocard <COUNT>`: Number of cards to request during registration
- `--exit`: Exit after displaying the current state (no interactive loop)
- `--listgames`: List available games and exit
- `--help`: Display help information
- `--version`: Display version information

**Default Behavior (No Game ID Specified):**
- Automatically calls `/gameslist` endpoint to display available games
- Shows game status, client counts, and game statistics
- Exits with instructions to use `--gameid <id>` to join a specific game

**Examples:**
```bash
# Start player client normally (shows games list and instructions)
cargo run --bin tombola-player

# Connect to a specific game with custom name and cards
cargo run --bin tombola-player -- --gameid game_12345678 --name "Player1" --nocard 3

# Display games list once and exit (non-interactive mode)
cargo run --bin tombola-player -- --exit

# Connect to specific game and exit after displaying status
cargo run --bin tombola-player -- --gameid game_12345678 --exit

# Explicitly list games and exit
cargo run --bin tombola-player -- --listgames

# Get help information
cargo run --bin tombola-player -- --help
```

**Notes about --exit option:**
- Provides non-interactive mode for monitoring games list and client status
- Displays available games list once and exits immediately when no game ID specified
- Displays current game state, cards, and achievements once and exits when in a specific game
- Useful for automation, status checking, or integration with other tools

**Card Generation Optimization:**
- The player client intelligently checks for existing card assignments before generating new cards
- If cards are already assigned to the client, card generation is skipped even if `--nocard` is specified
- This optimization reduces unnecessary server requests and improves performance for reconnecting clients
- Useful for automation, status checking, or integration with other tools
- Can be combined with other options like --name and --nocard

## HTTP API

The server provides a RESTful HTTP API on `http://127.0.0.1:3000` with **game-specific routing**. See `docs/TOMBOLA_API.md` for complete API documentation.

### Multi-Game API Features:
- **Game-Specific Endpoints**: All game operations use `/{game_id}/` routing for isolation
- **Client Registration**: `POST /{game_id}/register` - Register clients to specific games
- **Game Management**: `POST /newgame` - Create new games, `GET /gameslist` - List all games
- **Game Operations**: Extract numbers, manage cards, check status - all game-specific
- **Cross-Game Compatibility**: Clients can participate in multiple games simultaneously

### Key API Endpoints:
- `POST /newgame` - Create a new game (board client only)
- `GET /gameslist` - List all games with status and statistics
- `POST /{game_id}/register` - Register client to specific game
- `POST /{game_id}/extract` - Extract number (board client only)
- `GET /{game_id}/board` - Get game board state
- `GET /{game_id}/status` - Get game status and statistics
- `POST /{game_id}/dumpgame` - Save game state to JSON
- Card management endpoints under `/{game_id}/` routing

### Authentication & Authorization:
- Client authentication via `X-Client-ID` headers
- Board client (`0000000000000000`) has special privileges for game control
- Game-specific client isolation and validation

### Typical Workflow:
1. **Start Client Without Game ID**: Client automatically calls `/gameslist` to show available games
2. **Game Selection**: User selects an existing game or creates a new one
3. **Client Registration**: Client calls `/{game_id}/register` to join the selected game
4. **Game Participation**: Client interacts with game-specific endpoints for cards, board state, etc.
5. **Multi-Game Support**: Clients can participate in multiple games simultaneously

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
- **Multi-Game Registry**: `GameRegistry` manages multiple concurrent games with unique IDs
- **Game Super Struct**: Unified `Game` struct that encapsulates all game state components
- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal ID (format: `game_12345678`)
- **Game Status Tracking**: Games transition through New → Active → Closed states
- **Creation Timestamps**: Games include creation timestamps with human-readable formatting
- **Game Isolation**: Complete separation of client registrations, cards, and game state per game
- **Enhanced API Responses**: Game ID and creation time included in status and management endpoints

### Modular Components:
- **`game.rs`**: Unified game state management with ID and timestamp tracking, GameRegistry for multi-game support
- **`api_handlers.rs`**: Axum-based HTTP handlers with game-specific routing and comprehensive test suite
- **`config.rs`**: Configuration management with file-based settings
- **`logging.rs`**: Centralized logging with timestamp formatting
- **`extraction.rs`**: Shared extraction logic between server and API
- **`lib.rs`**: Library structure for shared functionality

### Thread-Safe State Management:
- Uses `Arc<Mutex<T>>` for coordinated access to shared game state
- Consistent mutex acquisition order to prevent deadlocks
- Game-specific state isolation through GameRegistry
- Shared state includes: Board, Pouch, ScoreCard, CardAssignmentManager, ClientRegistry
- Unified through the Game struct with proper coordination methods
- Multi-game concurrency support with thread-safe registry operations

## Dependencies

- `rand` - Random number generation for pouch extraction and game ID generation
- `crossterm` - Cross-platform terminal manipulation and keyboard input
- `tokio` - Async runtime with macros, rt-multi-thread, net, and time features
- `reqwest` - HTTP client with JSON support (for client binaries)
- `serde` - Serialization framework with derive features
- `serde_json` - JSON serialization support
- `axum` - Modern, ergonomic web framework built on hyper and tower
- `tower` - Middleware and service composition for HTTP services
- `tower-http` - HTTP-specific tower middleware with CORS support
- `chrono` - Date and time library for logging timestamps and game creation times
- `clap` - Command line argument parsing with derive features

## Development

See `.github/copilot-instructions.md` for detailed development guidelines and architectural patterns.

### Testing

The project includes comprehensive test coverage with:
- **API-Only Testing**: All tests use proper API handlers instead of bypassing internal methods
- **Multi-Game Test Scenarios**: Complex tests covering multiple games with multiple clients
- **Game Isolation Testing**: Verification that clients and games are properly isolated
- **Authentication Testing**: Proper validation of client authorization and board client privileges
- **Integration Testing**: Full client flow testing from registration to BINGO completion

**Test Quality Standards:**
- Tests interact exclusively through API endpoints (no internal method bypasses)
- Proper client authentication headers (`X-Client-ID`) in all test requests
- Game-specific routing validation in all multi-game scenarios
- Thread-safe test execution with proper state cleanup
- Comprehensive error handling and edge case coverage

Run tests with:
```bash
# Run all tests
cargo test

# Run API handler tests specifically
cargo test api_handlers --lib

# Run with output
cargo test -- --nocapture
```

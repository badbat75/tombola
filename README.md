# Tombola Game

A Rust-based multi-binary tombola (bingo) game with a client-server architecture, multi-game support, and comprehensive HTTP API using Axum web framework.

## Architecture

This project consists of three main binaries with a modular client library architecture:

- **`tombola-server`**: Main game server with terminal UI and HTTP API
- **`tombola-client`**: Board client that requires registration and displays current game state
- **`tombola-player`**: Interactive client for card management and gameplay

### Client Module Architecture

The clients are built using a modular architecture with shared functionality organized as library modules in `src/clients/`:

- **`common.rs`**: Shared data structures and HTTP utilities for API communication
- **`game_utils.rs`**: Game discovery, listing, and management utilities
- **`api_client.rs`**: Centralized HTTP API client functions with authentication support
- **`card_management.rs`**: Card-specific operations (generation, listing, assignment)
- **`registration.rs`**: Client registration and authentication utilities
- **`terminal.rs`**: Terminal UI utilities for board display and user interaction

This modular design eliminates code duplication between clients while maintaining clean separation of concerns.

*For detailed client architecture, features, and usage information, see [docs/CLIENTS.md](docs/CLIENTS.md).*

### Multi-Game Architecture

The server now supports multiple concurrent games through a **GameRegistry** system:
- **Game-Specific API Routing**: All API endpoints use `/{game_id}/` routing for game isolation
- **Client Registration Per Game**: All clients (including board clients) register to specific games using `/{game_id}/join`
- **Board Client Authorization**: Only registered board clients (client_type: "board") can extract numbers
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
  - **Modular Client Library**: Centralized shared functionality in `src/clients/` library
  - **Board Client Registration**: Board clients must register with client_type "board" to extract numbers
  - **Client Name Specification**: Both board and player clients support custom names via --name CLI option
  - Terminal-based board display client with CLI options and registration requirement
  - Interactive card management client with multi-game support
  - HTTP API integration with authentication via `X-Client-ID` headers
  - Smart game discovery and automatic game listing
  - Centralized API communication and error handling
  - Common data structures across all clients

*For detailed client features, CLI options, and usage examples, see [docs/CLIENTS.md](docs/CLIENTS.md).*

## Configuration

The game uses configurable card layouts and settings:
- Default: 2×3 grid of cards (6 cards total)
- Each card contains 5×3 numbers (15 numbers per card)
- Numbers range from 1-90 (calculated as FIRSTNUMBER to LASTNUMBER)
- Cards follow tombola rules with proper column distribution
- Server configuration: Host/port settings (default: 127.0.0.1:3000), enhanced logging system
- Client configuration: Connection settings with timeouts and retry logic
- File-based configuration support with fallback to defaults

### Logging Configuration

The server now includes an enhanced async logging system with multiple output modes:

- **Console Mode**: Logs output to terminal (default)
- **File Mode**: Logs written to module-specific files in `./logs/` directory
- **Both Mode**: Simultaneous console and file output
- **Module-Specific Files**: Separate log files for different components (e.g., `api_handlers.log`, `tombola_server.log`)
- **Configurable via `conf/server.conf`**: Set `logging = console|file|both` and `logpath = ./logs`

## Build and Run

```bash
# Build all binaries (server and clients)
cargo build --release

# Run main server (includes terminal UI and HTTP API)
cargo run --bin tombola-server

# Run display-only client
cargo run --bin tombola-client

# Run interactive card client
cargo run --bin tombola-player
```

*For detailed client CLI options and usage examples, see [docs/CLIENTS.md](docs/CLIENTS.md).*

## HTTP API

The server provides a RESTful HTTP API on `http://127.0.0.1:3000` with **game-specific routing**. See `docs/TOMBOLA_API.md` for complete API documentation.

### Multi-Game API Features:
- **Game-Specific Endpoints**: All game operations use `/{game_id}/` routing for isolation
- **Client Registration**: `POST /{game_id}/join` - Register clients to specific games with client types
- **Board Client Authorization**: Only clients with client_type "board" can extract numbers
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

*For client controls and interaction details, see [docs/CLIENTS.md](docs/CLIENTS.md).*

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
- `tokio` - Async runtime with macros, rt-multi-thread, net, and time features (also used for async logging system)
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

# Tombola Game Architecture

## Workspace Structure

The Tombola project is organized as a single Cargo crate with multiple binaries:

- **Main Crate (`tombola`)**: Contains the core game logic, server implementation, client applications, and shared libraries
- **Client Modules (`src/clients/`)**: Contains client applications and shared utilities as library modules within the main crate

This unified structure provides:
- **Clear Dependency Management**: Client modules can directly access core game types
- **Simplified Build Process**: All components build together as a single crate
- **Modular Development**: Client code is organized in dedicated modules
- **Easier Testing**: Client functionality can be tested as part of the main crate
- **Code Reuse**: Common client functionality is centralized in shared library modules
- **Reduced Duplication**: HTTP utilities, data structures, and API patterns are shared

*For detailed client architecture information, see [CLIENTS.md](CLIENTS.md).*

## Multi-Game Architecture Overview

The Tombola server now supports multiple concurrent games through a **GameRegistry** system, providing complete game isolation and independent state management.

### Core Architecture Components

#### Multi-Game Registry System (`src/game.rs`)
- **GameRegistry**: Central registry managing multiple concurrent games with unique IDs
- **Game Super Struct**: Unified `Game` struct encapsulating all game state components
- **Unique Game IDs**: 8-digit hexadecimal identifiers (format: `game_12345678`)
- **Game Status Tracking**: Games transition through New → Active → Closed states
- **Creation Timestamps**: Human-readable creation times for each game instance
- **Game Isolation**: Complete separation of client registrations, cards, and game state per game
- **Thread-Safe Registry**: Concurrent access to multiple games with proper mutex coordination

#### Game-Specific API Routing
- **Path-Based Routing**: All game operations use `/{game_id}/` routing for isolation
- **Client Registration Per Game**: Clients register to specific games using `/{game_id}/join`
- **Independent Game States**: Each game maintains separate Board, Pouch, ScoreCard, and Client registries
- **Game Management**: Create new games via `/newgame`, list all games via `/gameslist`
- **Cross-Game Client Support**: Clients can participate in multiple games simultaneously

### Game Configuration (`src/defs.rs`)
- `BOARDCONFIG`: Defines card layout (5×3 numbers, 2×3 card grid)
- `NUMBERSPERCARD`: 15 numbers per card (5 columns × 3 rows)
- Numbers range: 1-90 (calculated as `FIRSTNUMBER` to `LASTNUMBER`)
- `Colors`: Terminal color definitions for UI formatting (Green, Yellow, Red, Blue, Magenta)

### Game State Persistence (`src/game.rs`)
- **Automatic JSON Dumps**: Complete game state is automatically dumped to `data/games/` directory when:
  - BINGO is reached (game ends with score ≥ 15)
  - New game is started via `/newgame` endpoint (dumps incomplete games only - BINGO games already saved)
- **Manual Dumps**: Admin can trigger dumps via `/{game_id}/dumpgame` endpoint
- **File Format**: `game_{game_id}.json` with pretty-printed JSON
- **Complete State**: Includes board, pouch, scorecard, client registry, and card assignments
- **Security**: Only registered board clients (client_type "board") can trigger manual dumps

### Configuration Management (`src/config.rs`)
- `ServerConfig`: Host/port configuration with defaults (127.0.0.1:3000), enhanced logging system configuration
- `LoggingMode`: Enum supporting Console, File, and Both logging modes
- `ClientConfig`: Client connection settings including timeouts and retry logic
- File-based configuration support for both server and client settings
- Default configuration fallback when files are missing
- Logging path configuration with automatic directory creation

### Logging System (`src/logging.rs`)
- **Async Logging System**: Built with tokio channels for non-blocking log processing
- **Multiple Output Modes**: Console, File, or Both modes via `LoggingMode` enum
- **Module-Specific Log Files**: Separate log files for different components (e.g., `api_handlers.log`, `tombola_server.log`)
- **Centralized Configuration**: Integrated with `ServerConfig` for consistent settings
- **Log Levels**: Debug, Info, Warning, Error with proper level-based formatting
- **Automatic Timestamp Formatting**: Using chrono for consistent timestamp formatting
- **Thread-Safe File Writing**: Concurrent access to multiple log files with proper mutex coordination
- **Directory Management**: Automatic creation of log directories as needed

### Extraction Engine (`src/extraction.rs`)
- Core extraction logic separated from UI components
- Shared between terminal server and HTTP API
- Handles mutex coordination for thread-safe operations
- Validates game state before performing extractions

## Key Development Patterns

### HTTP API Server (`src/server.rs` + `src/api_handlers.rs`)
- **Multi-Game Axum Server**: Async server on `127.0.0.1:3000` with game-specific routing
- **Game-Specific Endpoints**: All game operations use `/{game_id}/` routing pattern for isolation
- **Game Management Endpoints**: `/newgame` for creation, `/gameslist` for discovery
- **Modular Architecture**: Separated API handlers in `api_handlers.rs` for maintainability
- **JSON API**: All endpoints return JSON with CORS headers via `tower-http`
- **Client Authentication**: Via `X-Client-ID` header with game-specific validation
- **Error Responses**: Standard HTTP status codes with custom `ApiError` type
- **Client Registration**: Restricted to pre-game state per individual game
- **AppState**: Dependency injection pattern with GameRegistry for multi-game support

### Smart Client Discovery
- **Automatic Game Listing**: Clients without specified game ID automatically call `/gameslist`
- **User Guidance**: Display available games with status and creation times
- **Interactive Instructions**: Provide clear guidance for game selection or creation
- **CLI Integration**: `--listgames` flag for explicit game discovery
- **Backward Compatibility**: Maintains existing behavior when game ID is specified

*For detailed client information, see [CLIENTS.md](CLIENTS.md).*

### Client Registry (`src/client.rs`)
- **Game-Specific Registration**: Thread-safe client registration per game instance
- **Registration Timing**: Prevents new client registration after game starts (numbers extracted)
- **Game State Validation**: Uses individual game board state to enforce registration restrictions
- **Multi-Game Support**: Clients can be registered to multiple games simultaneously
- **Client Isolation**: Complete separation of client data between different games

### Card Generation Algorithm (`src/card.rs`)
Cards are generated as groups of 6 with anti-adjacency rules:
- Each card has 15 numbers distributed across 9 columns (1-10, 11-20, ..., 81-90)
- Numbers are positioned to avoid adjacent placement across cards
- Use `CardManagement::generate_card_group()` for compliant card sets

### Terminal UI (`src/clients/terminal.rs`)
*For detailed terminal UI information, see [CLIENTS.md](CLIENTS.md).*

## Client Architecture

*For complete client architecture documentation, see [CLIENTS.md](CLIENTS.md).*

## File Organization

### Main Crate (`tombola`)
- `src/game.rs`: Multi-game registry and unified game state management with unique IDs and timestamps
- `src/defs.rs`: Core constants and type definitions
- `src/board.rs`: Game board state management
- `src/score.rs`: Scoring logic and prize calculations
- `src/card.rs`: Card generation and assignment logic
- `src/client.rs`: Game-specific client registration and management
- `src/server.rs`: Multi-game HTTP API server implementation (Axum-based)
- `src/api_handlers.rs`: Game-specific API handler functions with routing
- `src/pouch.rs`: Number extraction logic
- `src/config.rs`: Configuration management for server and client settings with enhanced logging configuration
- `src/logging.rs`: Async logging system with module-specific file output and multiple logging modes
- `src/extraction.rs`: Shared extraction logic for server and API
- `src/lib.rs`: Library structure with client modules for shared functionality
- `src/tombola_server.rs`: Main server binary with terminal UI
- `src/server_old.rs`: Legacy Hyper-based server implementation (deprecated)

### Configuration & Data Directories
- `conf/`: Configuration files including `server.conf` with logging settings and `client.conf`
- `data/games/`: JSON dumps of completed games (automatically created, git-ignored)
- `logs/`: Module-specific log files when file logging is enabled (automatically created, git-ignored)

### Client Modules (`src/clients/`)
*For detailed client module documentation, see [CLIENTS.md](CLIENTS.md).*

- `src/clients/tombola_client.rs`: Board display client binary with smart game discovery
- `src/clients/card_client.rs`: Interactive player client binary with multi-game support

# Tombola Game Architecture

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
- **Client Registration Per Game**: Clients register to specific games using `/{game_id}/register`
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
- **Security**: Only board client (ID: "0000000000000000") can trigger manual dumps

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

### Terminal UI (`src/terminal.rs`)
- Uses crossterm for cross-platform terminal control
- Color coding: Green for current number, Yellow for marked/winning numbers
- Board layout calculated with `downrightshift()` for proper spacing
- Interactive controls for clients:
  - `tombola-client`: ENTER to extract, F5 to refresh, ESC to exit
  - `tombola-server`: Any key to extract, ESC to exit
  - CLI support for game reset with `--newgame` option
- **Smart Discovery**: Clients automatically list available games when no game ID specified
- **Multi-Game CLI**: `--gameid <id>` for specific game selection, `--listgames` for discovery

## Client Architecture

### Board Client (`src/tombola_client.rs`)
- **Purpose**: Display-only client for monitoring game board state
- **Smart Discovery**: Automatically shows available games when no game ID specified
- **Game Creation**: Can create new games via `--newgame` flag (board client privileges)
- **Non-Interactive Mode**: `--exit` flag for single-state display and immediate exit
- **CLI Options**: Comprehensive command-line interface with help and version support

### Player Client (`src/card_client.rs`)
- **Purpose**: Interactive client for card management and gameplay
- **Smart Discovery**: Automatically shows available games with instructional messaging
- **Card Management**: Registration, card generation, and real-time game monitoring
- **Game Selection**: Must specify game ID to participate in specific games
- **Interactive Interface**: Menu-driven card viewing and game state monitoring

## File Organization

- `src/game.rs`: Multi-game registry and unified game state management with unique IDs and timestamps
- `src/defs.rs`: Core constants and type definitions
- `src/board.rs`: Game board state management
- `src/score.rs`: Scoring logic and prize calculations
- `src/card.rs`: Card generation and assignment logic
- `src/client.rs`: Game-specific client registration and management
- `src/server.rs`: Multi-game HTTP API server implementation (Axum-based)
- `src/api_handlers.rs`: Game-specific API handler functions with routing
- `src/terminal.rs`: Terminal UI rendering with smart discovery features
- `src/pouch.rs`: Number extraction logic
- `src/config.rs`: Configuration management for server and client settings
- `src/logging.rs`: Centralized logging utilities
- `src/extraction.rs`: Shared extraction logic for server and API
- `src/lib.rs`: Library structure for shared functionality
- `src/tombola_client.rs`: Board display client with smart game discovery
- `src/card_client.rs`: Interactive player client with multi-game support
- `src/tombola_server.rs`: Main server binary with terminal UI
- `src/server_old.rs`: Legacy Hyper-based server implementation (deprecated)

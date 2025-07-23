# Tombola Client Architecture

## Overview

The Tombola project includes multiple client applications built using a modular architecture with shared functionality organized as library modules in `src/clients/`.

This project consists of three main binaries:

- **`tombola-server`**: Main game server with terminal UI and HTTP API
- **`tombola-client`**: Board client that requires registration and displays current game state  
- **`tombola-player`**: Interactive client for card management and gameplay

## Client Library Modules

### Core Modules

- **`common.rs`**: Shared data structures (RegisterRequest, CardInfo, etc.) and basic HTTP utilities
- **`api_client.rs`**: Centralized HTTP API communication with error handling and authentication
- **`game_utils.rs`**: Game discovery, listing, and management utilities shared between clients
- **`card_management.rs`**: Card-specific operations like generation, listing, and assignment
- **`registration.rs`**: Client registration and authentication utilities
- **`terminal.rs`**: Terminal UI utilities for board display and user interaction

### Architecture Benefits

- **No Code Duplication**: Both clients share common functionality through centralized modules
- **Consistent API**: Unified HTTP communication patterns and error handling
- **Modular Design**: Each client uses only the modules it needs
- **Easy Maintenance**: Changes to API logic only need to be made in centralized modules
- **Type Safety**: Shared data structures ensure consistency across clients
- **Clean Separation**: Clear separation between UI logic and API communication

## Client Library Modules

The clients are built using a modular architecture with shared functionality organized as library modules in `src/clients/`:

- **`common.rs`**: Shared data structures and HTTP utilities for API communication
- **`game_utils.rs`**: Game discovery, listing, and management utilities
- **`api_client.rs`**: Centralized HTTP API client functions with authentication support
- **`card_management.rs`**: Card-specific operations (generation, listing, assignment)
- **`registration.rs`**: Client registration and authentication utilities
- **`terminal.rs`**: Terminal UI utilities for board display and user interaction

This modular design eliminates code duplication between clients while maintaining clean separation of concerns. Each client binary can import only the modules it needs from the main library (`use tombola::clients::{api_client, game_utils};`), creating a flexible and maintainable codebase.

**Module Structure**: Client modules are exposed as public library modules through the main crate, eliminating false "unused code" warnings and providing clearer dependency relationships.

## Build and Run

```bash
# Build all binaries (server and clients)
cargo build --release

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

- `--name <NAME>` / `-n <NAME>`: Board client name (default from config)
- `--newgame`: Create a new game before starting the client interface
- `--gameid <GAME_ID>`: Specify the game ID to connect to
- `--exit`: Exit after displaying the current state (no interactive loop)
- `--listgames`: List available games and exit
- `--help`: Display help information
- `--version`: Display version information

**Client Registration:**
- Board clients must register with client_type "board" before extracting numbers
- Registration happens automatically on startup with the specified or default name
- Only registered board clients can extract numbers from the game

**Default Behavior (No Game ID Specified):**
- Automatically calls `/gameslist` endpoint to display available games
- Shows game status, creation times, and statistics
- Exits with instructions to use `--gameid <id>` to join a specific game or `--newgame` to create one

**Examples:**
```bash
# Start board client normally (shows games list and instructions)
cargo run --bin tombola-client

# Start board client with custom name
cargo run --bin tombola-client -- --name "MainBoard" --gameid game_12345678

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
- Only registered board clients can create new games (must be registered with client_type "board")
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

- `--name <n>`: Set client name (overrides config file)
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

## Client Controls

### Board Client:
- **ENTER**: Extract a number from the pouch (when prompted)
- **F5**: Refresh the screen and update game state without extracting
- **ESC**: Exit the client

### Card Client:
- Interactive menu-driven interface for card management
- Card assignment and viewing capabilities
- Integration with HTTP API for real-time updates

### `common.rs` - Shared Data Structures & HTTP Utilities
- **Request/Response Types**: RegisterRequest, RegisterResponse, ErrorResponse, etc.
- **Card Data Structures**: CardInfo, AssignedCardInfo, GenerateCardsResponse
- **HTTP Utilities**: Generic functions for GET/POST requests with error handling
- **Authentication Support**: Client ID header management for API calls

### `game_utils.rs` - Game Management
- **Game Discovery**: Centralized game listing and selection logic
- **Server Testing**: Connection verification utilities
- **Common Patterns**: Shared game ID resolution and discovery workflows

### `api_client.rs` - API Communication
- **Game State APIs**: Board, scorecard, and pouch data retrieval
- **Player APIs**: Authenticated API calls for player-specific operations
- **Client Management**: Name resolution and client information utilities
- **Error Handling**: Consistent error patterns across all API calls

### `card_management.rs` - Card Operations
- **Card Generation**: Centralized card creation and management
- **Card Assignment**: Card listing and assignment utilities
- **Card Retrieval**: Individual card data access functions

### `registration.rs` - Client Authentication
- **Client Registration**: Centralized client registration with server
- **Authentication Handling**: Client ID management and server communication
- **Client Type Management**: Support for different client types (player, board, etc.)

### `terminal.rs` - Terminal UI Utilities
- **Display Functions**: Board rendering and terminal output formatting
- **User Input**: Key handling and interactive controls
- **Color Support**: Terminal color coding for enhanced user experience

## Client Applications

### Board Client (`src/clients/tombola_client.rs`)
- **Purpose**: Display-only client for monitoring game board state
- **Smart Discovery**: Automatically shows available games when no game ID specified
- **Game Creation**: Can create new games via `--newgame` flag (board client privileges)
- **Non-Interactive Mode**: `--exit` flag for single-state display and immediate exit
- **CLI Options**: Comprehensive command-line interface with help and version support

#### Interactive Controls
- ENTER: Extract a number using the /extract API endpoint
- F5: Refresh screen and re-fetch fresh data from server without extracting
- ESC: Exit the client application

#### CLI Options
- `--newgame`: Create a new game before starting the client
- `--gameid`: Specify the game ID to connect to
- `--listgames`: List active games and exit

### Player Client (`src/clients/card_client.rs`)
- **Purpose**: Interactive client for card management and gameplay
- **Smart Discovery**: Automatically shows available games with instructional messaging
- **Card Management**: Registration, card generation, and real-time game monitoring
- **Game Selection**: Must specify game ID to participate in specific games
- **Interactive Interface**: Menu-driven card viewing and game state monitoring

#### CLI Options
- `--name`: Override client name from configuration
- `--nocard`: Number of cards to request during registration
- `--exit`: Display current state once and exit
- `--gameid`: Specify game ID to connect to
- `--listgames`: List active games and exit

## Terminal UI Features

### Terminal UI (`src/clients/terminal.rs`)
- Uses crossterm for cross-platform terminal control
- Color coding: Green for current number, Yellow for marked/winning numbers
- Board layout calculated with `downrightshift()` for proper spacing
- Interactive controls for clients:
  - `tombola-client`: ENTER to extract, F5 to refresh, ESC to exit
  - `tombola-server`: Any key to extract, ESC to exit
  - CLI support for game reset with `--newgame` option
- **Smart Discovery**: Clients automatically list available games when no game ID specified
- **Multi-Game CLI**: `--gameid <id>` for specific game selection, `--listgames` for discovery

## Smart Client Discovery

- **Automatic Game Listing**: Clients without specified game ID automatically call `/gameslist`
- **User Guidance**: Display available games with status and creation times
- **Interactive Instructions**: Provide clear guidance for game selection or creation
- **CLI Integration**: `--listgames` flag for explicit game discovery
- **Backward Compatibility**: Maintains existing behavior when game ID is specified

## Multi-Game Support

### Game-Specific Features
- **Game-Specific API Routing**: All API endpoints use `/{game_id}/` routing for game isolation
- **Client Registration Per Game**: Clients register to specific games using `/{game_id}/join`
- **Independent Game States**: Each game maintains separate Board, Pouch, ScoreCard, and Client registries
- **Game Management**: Create new games via `/newgame` endpoint and list all games via `/gameslist`
- **Cross-Game Client Support**: Clients can participate in multiple games simultaneously

## Client Features

### Server Components (accessed by clients):
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

### Client Components:
- Multi-game discovery and selection with automatic game listing
- Interactive terminal-based user interface with color coding
- Smart game discovery when no game ID is specified
- Real-time board state monitoring and display
- Card management with registration and assignment
- Client authentication with server-provided unique IDs
- Cross-platform terminal support using crossterm
- Comprehensive CLI options for different use cases
- Error handling and connection retry logic
- Support for multiple concurrent game participation

## File Organization

### Client Modules (`src/clients/`)
- `src/clients/mod.rs`: Client module declarations exposed via main library
- `src/clients/common.rs`: **Common data structures and HTTP utilities**
  - Request/Response structures (RegisterRequest, RegisterResponse, etc.)
  - HTTP client utilities (get_json, post_json, etc.)
  - Card and game data structures
- `src/clients/game_utils.rs`: **Game management utilities**
  - Game discovery functions (list_games, get_game_id)
  - Server connection testing
  - Common game ID resolution patterns
- `src/clients/api_client.rs`: **HTTP API client utilities**
  - Game state API calls (get_board_data, get_scoremap, etc.)
  - Player-specific API calls with authentication
  - Client name resolution utilities
- `src/clients/card_management.rs`: **Card operations**
  - Card generation and management utilities
  - Card assignment and listing functions
- `src/clients/registration.rs`: **Client authentication**
  - Client registration and authentication utilities
- `src/clients/terminal.rs`: Terminal UI rendering with smart discovery features
- `src/clients/tombola_client.rs`: Board display client binary with smart game discovery
- `src/clients/card_client.rs`: Interactive player client binary with multi-game support

## Technical Architecture

### Module Import Pattern
Clients now import shared modules from the main library using:
```rust
use tombola::clients::{api_client, game_utils, common};
```

### Benefits
- **Clear Dependency Management**: Client modules can directly access core game types
- **Simplified Build Process**: All components build together as a single crate
- **Better Module Relationships**: The compiler properly understands how modules are connected
- **Reduced Warnings**: Eliminated false positive "unused items" warnings
- **Code Reuse**: Common client functionality is centralized in shared library modules
- **Modular Development**: Client code is organized in dedicated modules

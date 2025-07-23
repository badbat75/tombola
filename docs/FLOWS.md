# Tombola Game Flow Documentation

This document describes the interaction flows between the Tombola server and its clients, including sequence diagrams for different usage scenarios in the multi-game architecture.

## Multi-Game Architecture Overview

The server uses a **GameRegistry** system that supports multiple concurrent games:

- **Game-Specific Routing**: All game operations use `/{game_id}/` routing for complete isolation
- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal identifier (format: `game_12345678`)
- **Creation Timestamps**: Games include creation timestamps for tracking and debugging purposes
- **Thread-Safe Components**: Board, Pouch, ScoreCard, ClientRegistry, and CardAssignmentManager per game
- **Coordinated Access**: All components wrapped in `Arc<Mutex<T>>` for thread safety
- **Game Management**: `/newgame` for creation, `/gameslist` for discovery
- **Smart Client Discovery**: Automatic game listing when clients run without specified game ID

## Smart Client Discovery Flow

### Automatic Game Discovery

```mermaid
sequenceDiagram
    participant C as Client (no game ID)
    participant S as Tombola Server

    Note over C: Client starts without --gameid specified

    C->>S: GET /gameslist
    S-->>C: List of available games with status

    Note over C: Display games with creation times and status
    Note over C: Show instructions for game selection
    C-->>C: Exit with guidance message

    alt Board Client Instructions
        Note over C: "Please specify a game ID using --gameid <id>"
        Note over C: "or create a new game with --newgame"
    else Player Client Instructions
        Note over C: "Please specify a game ID using --gameid <id>"
        Note over C: "to join a specific game"
    end
```

### Explicit Game Listing

```mermaid
sequenceDiagram
    participant C as Client (--listgames)
    participant S as Tombola Server

    C->>S: GET /gameslist
    S-->>C: List of available games with detailed information

    Note over C: Display comprehensive game information:
    Note over C: - Game IDs and creation times
    Note over C: - Game status (New/Active/Closed)
    Note over C: - Client counts and statistics
    C-->>C: Exit after displaying list
```

## Multi-Game Client Registration and Game Flow

### Standard Interactive Flow with Game Selection

```mermaid
sequenceDiagram
    participant S as Tombola Server
    participant BC as Board Client
    participant PC as Player Client

    Note over S: Server with GameRegistry managing multiple games

    alt No Game ID Specified
        BC->>S: GET /gameslist
        S-->>BC: Available games list
        Note over BC: Display games and exit with instructions
    else Specific Game ID
        BC->>S: GET /{game_id}/status
        S-->>BC: Game state for specific game

        PC->>S: POST /{game_id}/join
        Note over PC: {"name": "player1", "client_type": "player"}
        S-->>PC: Registration successful (client_id)

        PC->>S: GET /{game_id}/listassignedcards
        Note over PC: Using X-Client-ID header
        S-->>PC: List of assigned cards for this game

        PC->>S: GET /{game_id}/getassignedcard/{card_id}
        S-->>PC: Card details and numbers

        Note over BC: User presses key to extract number
        BC->>S: POST /{game_id}/extract
        Note over BC: Using registered board client ID
        S-->>BC: Number extracted from pouch for this game

        BC->>S: GET /{game_id}/board
        S-->>BC: Updated board with extracted numbers

        BC->>S: GET /{game_id}/scoremap
        S-->>BC: Current scores and achievements for this game
    end
```

### Game Creation Flow

```mermaid
sequenceDiagram
    participant S as Tombola Server (GameRegistry)
    participant BC as Board Client

    BC->>S: POST /newgame
    Note over BC: Using registered board client ID
    S-->>BC: New game created with unique game_id

    Note over S: GameRegistry adds new game instance:
    Note over S: - New Game struct with unique ID
    Note over S: - Fresh Board, Pouch, ScoreCard
    Note over S: - Empty client registrations
    Note over S: - New timestamp

    BC->>S: GET /{new_game_id}/status
    S-->>BC: Fresh game state for new game

    BC->>S: GET /gameslist
    S-->>BC: Updated games list including new game
```

### Game Monitoring Flow

```mermaid
sequenceDiagram
    participant S as Tombola Server (GameRegistry)
    participant BC as Board Client
    participant PC as Player Client

    Note over PC: Monitoring loop every 2 seconds for specific game
    PC->>S: GET /{game_id}/board
    S-->>PC: Current extracted numbers for this game

    PC->>S: GET /{game_id}/scoremap
    S-->>PC: Current scorecard for this game

    PC->>S: GET /{game_id}/getassignedcard/{card_id}
    S-->>PC: Updated card info

    Note over PC: Display cards with highlights

    loop Until BINGO or Exit
        BC->>S: POST /{game_id}/extract
        S-->>BC: Next number for this game

        PC->>S: GET /{game_id}/board
        S-->>PC: Updated board for this game

        PC->>S: GET /{game_id}/scoremap
        S-->>PC: Updated scores for this game

        alt Achievement reached
            Note over S: Score updated (2,3,4,5 in line or BINGO) for this game
            PC->>S: GET /{game_id}/scoremap
            S-->>PC: Achievement confirmation for this game
        end
    end
```

### Multi-Game Concurrent Flow

```mermaid
sequenceDiagram
    participant S as Tombola Server (GameRegistry)
    participant BC1 as Board Client (Game A)
    participant PC1 as Player Client (Game A)
    participant BC2 as Board Client (Game B)
    participant PC2 as Player Client (Game B)

    Note over S: GameRegistry manages multiple games concurrently

    BC1->>S: POST /{game_a_id}/extract
    S-->>BC1: Number extracted for Game A

    BC2->>S: POST /{game_b_id}/extract
    S-->>BC2: Number extracted for Game B (independent)

    PC1->>S: GET /{game_a_id}/board
    S-->>PC1: Board state for Game A only

    PC2->>S: GET /{game_b_id}/board
    S-->>PC2: Board state for Game B only

    Note over S: Complete game isolation - no cross-contamination
```

### Non-Interactive Flow (--exit flag)

```mermaid
sequenceDiagram
    participant S as Tombola Server (GameRegistry)
    participant BC as Board Client (--exit)
    participant PC as Player Client (--exit)

    Note over S: Server running with multiple games in GameRegistry

    alt No Game ID Specified
        BC->>S: GET /gameslist
        S-->>BC: List of all available games
        Note over BC: Display games list once and exit
        BC-->>BC: Exit with instructions

        PC->>S: GET /gameslist
        S-->>PC: List of all available games
        Note over PC: Display games list once and exit
        PC-->>PC: Exit with instructions
    else Specific Game ID
        BC->>S: GET /{game_id}/status
        S-->>BC: Current game state for specific game

        BC->>S: GET /{game_id}/board
        S-->>BC: Current extracted numbers for specific game

        BC->>S: GET /{game_id}/scoremap
        S-->>BC: Current scorecard for specific game

        Note over BC: Display state once and exit
        BC-->>BC: Exit (no loop)

        PC->>S: POST /{game_id}/join
        Note over PC: {"name": "monitor", "client_type": "player"}
        S-->>PC: Registration successful for specific game

        PC->>S: GET /{game_id}/listassignedcards
        S-->>PC: Assigned cards for specific game

        PC->>S: GET /{game_id}/board
        S-->>PC: Extracted numbers for specific game

        PC->>S: GET /{game_id}/scoremap
        S-->>PC: Current scorecard for specific game

        PC->>S: GET /{game_id}/getassignedcard/{card_id}
        S-->>PC: Card details for specific game

        Note over PC: Display cards with highlights once and exit
        PC-->>PC: Exit (no monitoring loop)
    end
```

### Game Reset Flow (Multi-Game Context)

```mermaid
sequenceDiagram
    participant S as Tombola Server (GameRegistry)
    participant BC as Board Client (--newgame)

    Note over S: Server running with existing games in GameRegistry

    BC->>S: POST /newgame
    Note over BC: Using registered board client ID
    S-->>BC: New game created with unique game_id

    Note over S: GameRegistry creates new game instance:
    Note over S: - New Game struct with unique ID
    Note over S: - Fresh Board, Pouch, ScoreCard cleared
    Note over S: - Empty ClientRegistry and CardAssignments
    Note over S: - New creation timestamp
    Note over S: - Existing games remain untouched

    BC->>S: GET /{new_game_id}/status
    S-->>BC: Fresh game state for new game

    BC->>S: GET /gameslist
    S-->>BC: Updated list including new game

    alt --exit flag used
        Note over BC: Display new game state once and exit
        BC-->>BC: Exit
    else Interactive mode
        Note over BC: Start normal monitoring loop for new game
        loop Game interaction
            BC->>S: Various /{new_game_id}/ API calls
            S-->>BC: Responses for specific game
        end
    end
```

## Client Authentication

### Client Authentication and Registration

- **Board Clients**: Register with client_type "board"
  - Must register to each game they want to manage via `/{game_id}/join`
  - Receive special BOARD_ID card (`"0000000000000000"`) during registration
  - Can perform extractions via `/{game_id}/extract`
  - Can create new games via `/newgame`
  - Can dump game state via `/{game_id}/dumpgame`

- **Player Clients**: Register with client_type "player"
  - Must register via `/{game_id}/join` before accessing game-specific endpoints
  - Receive regular numbered cards during registration
  - Cannot extract numbers or create games
  - Can only access their own assigned cards within specific games
  - Must register separately for each game they want to join

### Multi-Game Header Authentication

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server (GameRegistry)

    C->>S: Request with X-Client-ID header to /{game_id}/endpoint
    alt Valid Client ID for specific game
        S-->>C: Success response with game-specific data
    else Invalid/Missing Client ID
        S-->>C: 401 Unauthorized
    else Client not registered to this game
        S-->>C: 403 Forbidden
    end
```

## API Endpoint Categories

### Global Endpoints (No Game ID Required)
- `POST /newgame` - Create new game (board client only)
- `GET /gameslist` - List all available games
- `GET /clientinfo` - Get client info by name (global search)
- `GET /clientinfo/{client_id}` - Get client info by ID (global search)

### Game-Specific Public Endpoints (No Authentication Required)
- `GET /{game_id}/status` - Server and specific game status
- `GET /{game_id}/board` - Current extracted numbers for game
- `GET /{game_id}/pouch` - Remaining numbers in pouch for game
- `GET /{game_id}/scoremap` - Current scorecard and achievements for game
- `POST /{game_id}/join` - Client registration to specific game

### Game-Specific Authenticated Endpoints (Require X-Client-ID)
- `POST /{game_id}/extract` - Extract number from pouch (Board Client only)
- `POST /{game_id}/dumpgame` - Dump game state to JSON (Board Client only)
- `POST /{game_id}/generatecards` - Generate cards for client in game
- `GET /{game_id}/listassignedcards` - List client's assigned cards in game
- `GET /{game_id}/getassignedcard/{card_id}` - Get specific card details in game

## Error Handling

### Common Error Scenarios

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server (GameRegistry)

    alt Registration after extraction in specific game
        C->>S: POST /{game_id}/join
        Note over S: Numbers already extracted in this game
        S-->>C: 409 Conflict
    end

    alt Missing authentication for game-specific endpoint
        C->>S: Authenticated /{game_id}/endpoint without X-Client-ID
        S-->>C: 401 Unauthorized
    end

    alt Invalid client ID for game
        C->>S: Request with invalid X-Client-ID to /{game_id}/endpoint
        S-->>C: 401 Unauthorized
    end

    alt Client not registered to specific game
        C->>S: Request to /{game_id}/endpoint
        Note over S: Valid client ID but not registered to this game
        S-->>C: 403 Forbidden
    end

    alt Extraction by non-board client
        C->>S: POST /{game_id}/extract
        Note over C: Using regular client ID
        S-->>C: 403 Forbidden
    end

    alt Invalid game ID
        C->>S: Request to /{invalid_game_id}/endpoint
        S-->>C: 404 Not Found
    end
```

## Configuration and Deployment

### Server Configuration
- **Host**: 127.0.0.1 (localhost only)
- **Port**: 3000
- **Protocol**: HTTP/1.1
- **Runtime**: Tokio async runtime
- **Architecture**: Multi-game with GameRegistry

### Client Configuration
- **Connection timeout**: 30 seconds
- **Server URL**: Configurable via client config files
- **Client names**: Configurable via CLI or config files
- **Game Selection**: Via `--gameid` CLI option or automatic discovery

### Thread Safety
- **GameRegistry**: Thread-safe access to multiple games
- **Per-Game State**: All shared state uses `Arc<Mutex<T>>` for thread-safe access
- **Coordinated Access**: Proper mutex acquisition order prevents deadlocks
- **Game Isolation**: Complete separation of game state between different games

## Game State Persistence

The Tombola server includes automatic game state persistence features:

### Automatic JSON Dumps
Game state is automatically saved to `data/games/` directory in the following scenarios:

```mermaid
sequenceDiagram
    participant S as Tombola Server (GameRegistry)
    participant FS as File System

    alt BINGO Achieved in specific game
        Note over S: Score reaches 15 (BINGO) in game_12345678
        S->>FS: Save complete game to game_12345678.json
        Note over FS: Game marked as completed
    end

    alt New Game Started
        Note over S: POST /newgame called
        alt Multiple incomplete games exist
            S->>FS: Save each incomplete game to game_{id}.json
            Note over FS: Incomplete games preserved
        end
        Note over S: Initialize new game with new ID in GameRegistry
    end

    alt Manual Dump Requested
        Note over S: POST /{game_id}/dumpgame called (Board Client only)
        S->>FS: Save specific game to game_{game_id}.json
        Note over FS: Manual snapshot created for specific game
    end
```

### File Format
- **Location**: `data/games/` directory
- **Naming**: `game_{game_id}.json` format (e.g., `game_12345678.json`)
- **Content**: Complete game state including:
  - Board state (extracted numbers)
  - Pouch state (remaining numbers)
  - ScoreCard (current scores and achievements)
  - Client registry (all registered clients for this game)
  - Card assignments (all assigned cards for this game)
- **Format**: Pretty-printed JSON for human readability
- **Game Isolation**: Each game file contains only that game's data

### Security Considerations
- Only registered board clients (client_type "board") can trigger manual dumps via `/{game_id}/dumpgame`
- Automatic dumps occur without authentication requirements
- Game files are stored locally in the server's file system
- Each game's data is completely isolated from other games

### Integration with External Tools
Non-interactive mode allows integration with:
- Monitoring dashboards showing multiple games
- Automation scripts for game management
- CI/CD pipelines for testing multiple game scenarios
- External notifications systems for multi-game events

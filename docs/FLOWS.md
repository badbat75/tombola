# Tombola Game Flow Documentation

This document describes the interaction flows between the Tombola server and its clients, including sequence diagrams for different usage scenarios.

### Game State Management

The server uses a unified **Game super struct** that encapsulates all game state:

- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal identifier (format: `game_12345678`)
- **Creation Timestamps**: Games include creation timestamps for tracking and debugging purposes
- **Thread-Safe Components**: Board, Pouch, ScoreCard, ClientRegistry, and CardAssignmentManager
- **Coordinated Access**: All components wrapped in `Arc<Mutex<T>>` for thread safety
- **Enhanced API**: Game ID and creation time included in status and reset responses

## Client Registration and Game Flow

### Standard Interactive Flow

```mermaid
sequenceDiagram
    participant S as Tombola Server
    participant BC as Board Client
    participant PC as Player Client

    Note over S: Server starts with clean game state

    BC->>S: GET /status
    S-->>BC: Game state (no extractions yet)

    PC->>S: POST /register
    Note over PC: {"name": "player1", "client_type": "player"}
    S-->>PC: Registration successful (client_id)

    PC->>S: GET /listassignedcards
    Note over PC: Using X-Client-ID header
    S-->>PC: List of assigned cards

    PC->>S: GET /getassignedcard/{card_id}
    S-->>PC: Card details and numbers

    Note over BC: User presses key to extract number
    BC->>S: POST /extract
    Note over BC: Using special client ID "0000000000000000"
    S-->>BC: Number extracted from pouch

    BC->>S: GET /board
    S-->>BC: Updated board with extracted numbers

    BC->>S: GET /scoremap
    S-->>BC: Current scores and achievements

    Note over PC: Monitoring loop every 2 seconds
    PC->>S: GET /board
    S-->>PC: Current extracted numbers

    PC->>S: GET /scoremap
    S-->>PC: Current scorecard

    PC->>S: GET /getassignedcard/{card_id}
    S-->>PC: Updated card info

    Note over PC: Display cards with highlights

    loop Until BINGO or Exit
        BC->>S: POST /extract
        S-->>BC: Next number

        PC->>S: GET /board
        S-->>PC: Updated board

        PC->>S: GET /scoremap
        S-->>PC: Updated scores

        alt Achievement reached
            Note over S: Score updated (2,3,4,5 in line or BINGO)
            PC->>S: GET /scoremap
            S-->>PC: Achievement confirmation
        end
    end
```

### Non-Interactive Flow (--exit flag)

```mermaid
sequenceDiagram
    participant S as Tombola Server
    participant BC as Board Client (--exit)
    participant PC as Player Client (--exit)

    Note over S: Server running with existing game state

    BC->>S: GET /status
    S-->>BC: Current game state

    BC->>S: GET /board
    S-->>BC: Current extracted numbers

    BC->>S: GET /scoremap
    S-->>BC: Current scorecard

    Note over BC: Display state once and exit
    BC-->>BC: Exit (no loop)

    PC->>S: POST /register
    Note over PC: {"name": "monitor", "client_type": "player"}
    S-->>PC: Registration successful

    PC->>S: GET /listassignedcards
    S-->>PC: Assigned cards

    PC->>S: GET /board
    S-->>PC: Extracted numbers

    PC->>S: GET /scoremap
    S-->>PC: Current scorecard

    PC->>S: GET /getassignedcard/{card_id}
    S-->>PC: Card details

    Note over PC: Display cards with highlights once and exit
    PC-->>PC: Exit (no monitoring loop)
```

### Game Reset Flow

```mermaid
sequenceDiagram
    participant S as Tombola Server
    participant BC as Board Client (--newgame)

    Note over S: Server running with existing game state

    BC->>S: POST /newgame
    Note over BC: Using special client ID "0000000000000000"
    S-->>BC: Game reset confirmation

    Note over S: All components reset:
    Note over S: - Board cleared
    Note over S: - Pouch refilled
    Note over S: - ScoreCard reset
    Note over S: - Cards cleared
    Note over S: - New game ID generated

    BC->>S: GET /status
    S-->>BC: Fresh game state

    BC->>S: GET /runninggameid
    S-->>BC: New game ID and timestamp

    alt --exit flag used
        Note over BC: Display state once and exit
        BC-->>BC: Exit
    else Interactive mode
        Note over BC: Start normal monitoring loop
        loop Game interaction
            BC->>S: Various API calls
            S-->>BC: Responses
        end
    end
```

## Client Authentication

### Special Client IDs

- **Board Client**: Uses special client ID `"0000000000000000"` (16 zeros)
  - Can perform extractions via `/extract`
  - Can reset game via `/newgame`
  - No registration required

- **Player Clients**: Use dynamically generated 16-character hexadecimal IDs
  - Must register via `/register` before accessing other endpoints
  - Cannot extract numbers or reset games
  - Can only access their own assigned cards

### Header Authentication

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server

    C->>S: Request with X-Client-ID header
    alt Valid Client ID
        S-->>C: Success response
    else Invalid/Missing Client ID
        S-->>C: 401 Unauthorized
    end
```

## API Endpoint Categories

### Public Endpoints (No Authentication Required)
- `GET /status` - Server and game status
- `GET /runninggameid` - Current game ID and creation time
- `GET /board` - Current extracted numbers
- `GET /pouch` - Remaining numbers in pouch
- `GET /scoremap` - Current scorecard and achievements
- `POST /register` - Client registration

### Authenticated Endpoints (Require X-Client-ID)
- `POST /extract` - Extract number from pouch (Board Client only)
- `POST /newgame` - Reset game state (Board Client only)
- `POST /dumpgame` - Dump game state to JSON (Board Client only)
- `GET /clientinfo` - Get client info by name
- `GET /clientinfo/{client_id}` - Get client info by ID
- `POST /generatecards` - Generate cards for client
- `GET /listassignedcards` - List client's assigned cards
- `GET /getassignedcard/{card_id}` - Get specific card details

## Error Handling

### Common Error Scenarios

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server

    alt Registration after extraction
        C->>S: POST /register
        Note over S: Numbers already extracted
        S-->>C: 409 Conflict
    end

    alt Missing authentication
        C->>S: Authenticated endpoint without X-Client-ID
        S-->>C: 401 Unauthorized
    end

    alt Invalid client ID
        C->>S: Request with invalid X-Client-ID
        S-->>C: 401 Unauthorized
    end

    alt Extraction by non-board client
        C->>S: POST /extract
        Note over C: Using regular client ID
        S-->>C: 403 Forbidden
    end
```

## Configuration and Deployment

### Server Configuration
- **Host**: 127.0.0.1 (localhost only)
- **Port**: 3000
- **Protocol**: HTTP/1.1
- **Runtime**: Tokio async runtime

### Client Configuration
- **Connection timeout**: 30 seconds
- **Server URL**: Configurable via client config files
- **Client names**: Configurable via CLI or config files

### Thread Safety
- All shared state uses `Arc<Mutex<T>>` for thread-safe access
- Coordinated mutex acquisition order prevents deadlocks
- Unified Game struct manages all components

## Game State Persistence

The Tombola server includes automatic game state persistence features:

### Automatic JSON Dumps
Game state is automatically saved to `data/games/` directory in the following scenarios:

```mermaid
sequenceDiagram
    participant S as Tombola Server
    participant FS as File System

    alt BINGO Achieved
        Note over S: Score reaches 15 (BINGO)
        S->>FS: Save complete game to {game_id}_{timestamp}.json
        Note over FS: Game marked as completed
    end

    alt New Game Started
        Note over S: POST /newgame called
        alt Incomplete game exists
            S->>FS: Save current game to {game_id}_{timestamp}.json
            Note over FS: Incomplete game preserved
        end
        Note over S: Initialize new game with new ID
    end

    alt Manual Dump Requested
        Note over S: POST /dumpgame called (Board Client only)
        S->>FS: Save current game to {game_id}_{timestamp}.json
        Note over FS: Manual snapshot created
    end
```

### File Format
- **Location**: `data/games/` directory
- **Naming**: `{game_id}_{timestamp}.json` format
- **Content**: Complete game state including:
  - Board state (extracted numbers)
  - Pouch state (remaining numbers)
  - ScoreCard (current scores and achievements)
  - Client registry (all registered clients)
  - Card assignments (all assigned cards)
- **Format**: Pretty-printed JSON for human readability

### Security Considerations
- Only the Board Client (ID: "0000000000000000") can trigger manual dumps via `/dumpgame`
- Automatic dumps occur without authentication requirements
- Game files are stored locally in the server's file system

### Integration with External Tools
Non-interactive mode allows integration with:
- Monitoring dashboards
- Automation scripts
- CI/CD pipelines
- External notifications systems

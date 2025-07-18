# Tombola Game Flow Documentation

This document describes the interaction flows between the Tombola server and its clients, including sequence diagrams for different usage scenarios.

## Architecture Overview

The Tombola game consists of three main components:

1. **Tombola Server** (`tombola-server`): Main game server with terminal UI and HTTP API
2. **Board Client** (`tombola-client`): Terminal client for displaying game state and performing extractions
3. **Player Client** (`tombola-player`): Interactive client for card management and gameplay

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
- `GET /client/{name}` - Get client info by name
- `GET /clientbyid/{id}` - Get client info by ID
- `POST /generatecardsforme` - Generate cards for client
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

## Usage Patterns

### Automation and Monitoring
The `--exit` flag enables non-interactive usage patterns:

```bash
# Status monitoring script
while true; do
    cargo run --bin tombola-client -- --exit
    sleep 30
done

# Player monitoring
cargo run --bin tombola-player -- --name "Monitor" --exit > game_state.txt
```

### Integration with External Tools
Non-interactive mode allows integration with:
- Monitoring dashboards
- Automation scripts
- CI/CD pipelines
- External notifications systems

### Development and Testing
The `--newgame` flag combined with `--exit` enables:
- Automated testing scenarios
- Development environment setup
- Game state reset in scripts

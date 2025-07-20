# Tombola API Documentation

## Overview

The Tombola API provides a RESTful HTTP interface for managing tombola games, client registration, and card management. The server runs on `http://127.0.0.1:3000` and uses JSON for request/response payloads.

## Game Architecture

The Tombola server uses a unified **Game super struct** that encapsulates all game state components:

- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal identifier (format: `game_12345678`)
- **Creation Timestamps**: Games include creation timestamps for tracking and debugging purposes
- **Thread-Safe Components**: All game state is managed through coordinated `Arc<Mutex<T>>` access
- **State Components**: Board, Pouch, ScoreCard, ClientRegistry, and CardAssignmentManager
- **Enhanced Responses**: Game ID and creation time are included in status and reset API responses

## Base URL
```
http://127.0.0.1:3000
```
## Authentication

Most endpoints require client authentication via the `X-Client-ID` header. Clients must register first to obtain a client ID.

## Common Headers

- `Content-Type: application/json` (for POST requests)
- `X-Client-ID: <client_id>` (for authenticated endpoints)
- `Access-Control-Allow-Origin: *` (included in all responses)

## Data Types

- `Number`: 8-bit unsigned integer (u8) representing tombola numbers (1-90)
- `Card`: 3x9 grid where each cell can contain a number or be empty (represented as `Option<u8>`)
- `ScoreAchievement`: Object containing achievement details
  - `client_id`: String identifier for the client that achieved the score
  - `card_id`: String identifier for the card that achieved the score
  - `numbers`: Array of numbers that directly contributed to the achievement

## Error Responses

All endpoints may return error responses with the following structure:

```json
{
  "error": "Error message description"
}
```

Common HTTP status codes:
- `400 Bad Request`: Invalid request format or missing required fields
- `401 Unauthorized`: Client not registered or invalid client ID
- `403 Forbidden`: Access denied to requested resource
- `404 Not Found`: Resource not found
- `409 Conflict`: Resource already exists or conflict with current state
- `500 Internal Server Error`: Server-side error

## API Endpoints Index

### Client Registry
| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/register` | Register a new client |
| `GET` | `/client/{client_name}` | Get client information by name |
| `GET` | `/clientbyid/{client_id}` | Get client information by ID |

### Card Management
| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/generatecardsforme` | Generate cards for authenticated client |
| `GET` | `/listassignedcards` | List all assigned cards |
| `GET` | `/getassignedcard/{card_id}` | Get specific card by ID |

### Board & Game State
| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/board` | Get extracted numbers |
| `GET` | `/pouch` | Get remaining numbers |
| `GET` | `/status` | Get overall game status |
| `POST` | `/extract` | Extract next number (board client only) |

### Score Management
| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/scoremap` | Get current scores and achievements |

### Game Administration
| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/runninggameid` | Get current game ID and creation time |
| `POST` | `/newgame` | Reset game state (board client only) |
| `POST` | `/dumpgame` | Dump game state to JSON (board client only) |

## Endpoints

### 1. Client Registration

#### POST /register

Register a new client with the tombola server.

**Request Body:**
```json
{
  "name": "client_name",
  "client_type": "player|admin|viewer",
  "nocard": 6  // Optional: number of cards to generate during registration (default: 1)
}
```

**Success Response (200 OK):**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "message": "Client 'client_name' registered successfully"
}
```

**Error Response - Registration After Game Started (409 Conflict):**
```json
{
  "error": "Cannot register new clients after numbers have been extracted"
}
```

**Notes:**
- **Game State Restriction**: New clients can only be registered when no numbers have been extracted from the pouch
- Once the first number is extracted, all new registration attempts will fail with a 409 Conflict error
- This ensures fair play by preventing players from joining mid-game
- If `nocard` is not specified, the server will automatically generate 1 card for the client by default
- If `nocard` is specified, the server will generate and assign the requested number of cards to the client
- If client already exists, returns existing client information
- Client ID is generated using a hash of name, type, and timestamp

### 2. Client Information

#### GET /client/{client_name}

Retrieve information about a registered client by name.

**Path Parameters:**
- `client_name`: Name of the client to retrieve

**Response:**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "name": "client_name",
  "client_type": "player",
  "registered_at": "SystemTime representation"
}
```

#### GET /clientbyid/{client_id}

Retrieve information about a registered client by client ID.

**Path Parameters:**
- `client_id`: Client ID of the client to retrieve

**Response:**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "name": "client_name",
  "client_type": "player",
  "registered_at": "SystemTime representation"
}
```

**Notes:**
- Returns the same information as `/client/{client_name}` but uses client ID for lookup
- Useful for resolving client names from client IDs in ScoreAchievement data

### 3. Card Management

#### POST /generatecardsforme

Generate cards for a registered client.

**Headers:**
- `X-Client-ID: <client_id>` (required)

**Request Body:**
```json
{
  "count": 6  // Number of cards to generate (1-6)
}
```

**Response:**
```json
{
  "cards": [
    {
      "card_id": "unique_card_id",
      "card_data": [
        [null, 15, null, 37, null, 52, null, 68, 89],
        [4, null, 23, null, 41, null, 67, null, null],
        [null, 19, null, 39, null, 58, null, 74, 90]
      ]
    }
  ],
  "message": "Generated 6 cards successfully"
}
```

**Notes:**
- Card generation is only allowed during registration or if client has no existing cards
- Each card is a 3x9 grid following tombola rules
- `null` represents empty cells in the card
- Cards are generated in groups of 6 with anti-adjacency patterns

#### GET /listassignedcards

List all cards assigned to a client.

**Headers:**
- `X-Client-ID: <client_id>` (required)

**Response:**
```json
{
  "cards": [
    {
      "card_id": "card_id_1",
      "assigned_to": "A1B2C3D4E5F6G7H8"
    },
    {
      "card_id": "card_id_2",
      "assigned_to": "A1B2C3D4E5F6G7H8"
    }
  ]
}
```

#### GET /getassignedcard/{card_id}

Retrieve a specific card assigned to a client.

**Path Parameters:**
- `card_id`: ID of the card to retrieve

**Headers:**
- `X-Client-ID: <client_id>` (required)

**Response:**
```json
{
  "card_id": "card_id_1",
  "card_data": [
    [null, 15, null, 37, null, 52, null, 68, 89],
    [4, null, 23, null, 41, null, 67, null, null],
    [null, 19, null, 39, null, 58, null, 74, 90]
  ]
}
```

**Notes:**
- Only the client who owns the card can retrieve it
- Returns `403 Forbidden` if card belongs to another client

### 4. Game State

#### GET /board

Get the current board state (numbers that have been extracted).

**Response:**
```json
{
  "numbers": [15, 23, 37, 41, 52, 68, 74, 89],
  "marked_numbers": [15, 23, 37, 41, 52, 68, 74, 89]
}
```

**Notes:**
- Returns Board struct with numbers array (in extraction order) and marked_numbers set
- Empty arrays if no numbers have been extracted yet

#### GET /pouch

Get the current pouch state (remaining numbers).

**Response:**
```json
{
  "numbers": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 38, 39, 40, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 69, 70, 71, 72, 73, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 90]
}
```

**Notes:**
- Returns Pouch struct directly with numbers array
- `numbers` contains all numbers that haven't been extracted yet
- The count of remaining numbers can be obtained from the length of the numbers array

#### GET /scoremap

Get the current scorecard and score map (prize tracking information).

**Response:**
```json
{
  "published_score": 5,
  "score_map": {
    "2": [
      {
        "client_id": "A1B2C3D4E5F6G7H8",
        "card_id": "card_abc123",
        "numbers": [15, 23]
      }
    ],
    "3": [
      {
        "client_id": "A1B2C3D4E5F6G7H8",
        "card_id": "card_abc123",
        "numbers": [15, 23, 37]
      },
      {
        "client_id": "0000000000000000",
        "card_id": "0000000000000000",
        "numbers": [15, 23, 37]
      }
    ],
    "5": [
      {
        "client_id": "0000000000000000",
        "card_id": "0000000000000000",
        "numbers": [15, 23, 37, 41, 52]
      }
    ]
  }
}
```

**Notes:**
- Returns ScoreCard struct with `published_score` and `score_map` fields
- `published_score`: The highest score achieved so far (current published achievement level)
- `score_map`: Map of score indices to arrays of ScoreAchievement objects
- Each ScoreAchievement contains:
  - `client_id`: The ID of the client who achieved the score (or "0000000000000000" for board achievements)
  - `card_id`: The ID of the card that achieved the score (or "0000000000000000" for board achievements)
  - `numbers`: Array of specific numbers that contributed to achieving that score level
- Returns `published_score: 0` if no achievements have been recorded yet
- Each key in score_map represents a score level:
  - `2`, `3`, `4`, `5`: Number of numbers in a line achievement
  - `15`: BINGO (full card completion)
- For line achievements, `numbers` contains only the numbers from the winning line
- For BINGO achievements, `numbers` contains all 15 numbers that completed the card
- Empty score_map `{}` if no scores have been recorded yet
- To get client names from client IDs, use the `/clientbyid/{client_id}` endpoint

#### GET /status

Get overall server status and game information.

**Response:**
```json
{
  "status": "running",
  "game_id": "game_12345678",
  "created_at": "2025-07-17 14:30:25 UTC",
  "numbers_extracted": 8,
  "scorecard": 5,
  "server": "tokio-hyper"
}
```

**Notes:**
- `game_id`: Unique 8-digit hexadecimal identifier for the current game instance
- `created_at`: Human-readable timestamp when the current game was created/reset
- `numbers_extracted`: Total count of numbers extracted so far
- `scorecard`: Current published score (highest achievement level reached)
- `server`: Server implementation identifier

#### POST /extract

Extract the next number from the pouch (remote extraction control).

**Authentication Required:** Yes (X-Client-ID header must be "0000000000000000")

**Authorization:** Only the board client with ID "0000000000000000" can extract numbers.

**Request:**
```bash
curl -X POST http://127.0.0.1:3000/extract \
  -H "X-Client-ID: 0000000000000000" \
  -H "Content-Type: application/json"
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "extracted_number": 42,
  "numbers_remaining": 82,
  "total_extracted": 8,
  "message": "Number 42 extracted successfully"
}
```

**Error Response - Unauthorized Client (403 Forbidden):**
```json
{
  "error": "Unauthorized: Only board client can extract numbers"
}
```

**Error Response - Pouch Empty (409 Conflict):**
```json
{
  "error": "No numbers remaining in pouch"
}
```

**Error Response - Authentication (400 Bad Request):**
```json
{
  "error": "Client ID header (X-Client-ID) is required"
}
```

**Notes:**
- Performs the same extraction logic as manual `next_extraction` in the terminal UI
- **Security**: Only the special board client (ID: "0000000000000000") is authorized to extract numbers
- Regular game clients cannot trigger extractions for security and game integrity
- Automatically updates the board state, scorecard, and marked numbers
- Follows the coordinated mutex locking pattern to ensure thread safety
- Returns detailed information about the extraction result
- `numbers_remaining`: Count of numbers still available in the pouch
- `total_extracted`: Total numbers extracted so far (including this one)
- Server logs the extraction with client identification for audit purposes

#### POST /newgame

Reset all game state to start a new game.

**Authentication Required:** Yes (X-Client-ID header must be "0000000000000000")

**Authorization:** Only the board client with ID "0000000000000000" can reset the game.

**Request:**
```bash
curl -X POST http://127.0.0.1:3000/newgame \
  -H "X-Client-ID: 0000000000000000" \
  -H "Content-Type: application/json"
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "message": "New game started successfully",
  "game_id": "game_87654321",
  "created_at": "2025-07-17 14:35:12 UTC",
  "reset_components": [
    "Board state cleared",
    "Pouch refilled with numbers 1-90",
    "Score card reset",
    "Card assignments cleared"
  ]
}
```

**Error Response - Unauthorized Client (403 Forbidden):**
```json
{
  "error": "Unauthorized: Only board client can reset the game"
}
```

**Error Response - Authentication (400 Bad Request):**
```json
{
  "error": "Client ID header (X-Client-ID) is required"
}
```

**Notes:**
- Resets all shared game state to initial conditions and generates a new unique game ID
- **Game Instance Tracking**: Each new game gets a unique 8-digit hexadecimal ID and fresh timestamp
- **Security**: Only the special board client (ID: "0000000000000000") is authorized to reset the game
- Clears the Board (extracted numbers and marked positions)
- Refills the Pouch with all numbers from 1-90
- Resets the ScoreCard to initial state (published_score: 0, empty score_map)
- Clears all card assignments from CardAssignmentManager
- Clears all registered clients from the client registry
- **After reset**: New clients can register again since no numbers have been extracted
- Follows the coordinated mutex locking pattern to ensure thread safety
- Server logs the game reset with client identification for audit purposes
- All clients will need to re-register and obtain new card assignments after a game reset
- **Selective Auto-Dump**: Before resetting, incomplete games (started but no BINGO) are automatically dumped to JSON file in `data/games/` directory. BINGO games are not re-dumped since they were already saved when BINGO occurred.

#### POST /dumpgame

Manually dump the current game state to a JSON file for analysis, auditing, or debugging.

**Authentication Required:** Yes (X-Client-ID header must be "0000000000000000")

**Authorization:** Only the board client with ID "0000000000000000" can dump game state.

**Request:**
```bash
curl -X POST http://127.0.0.1:3000/dumpgame \
  -H "X-Client-ID: 0000000000000000" \
  -H "Content-Type: application/json"
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Game dumped to: data/games/game_87654321_20250717_143022.json",
  "game_id": "game_87654321",
  "game_ended": false,
  "bingo_reached": false,
  "pouch_empty": false
}
```

**Error Response - Unauthorized Client (403 Forbidden):**
```json
{
  "error": "Unauthorized: Only board client can dump the game"
}
```

**Error Response - File System Error (500 Internal Server Error):**
```json
{
  "error": "Failed to dump game: Failed to create directory \"data/games\": Permission denied"
}
```

**Notes:**
- Dumps complete game state including board, pouch, scorecard, client registry, and card assignments
- Files are saved in `data/games/` directory with format: `{game_id}_{timestamp}.json`
- Can be called at any time during the game, not just when the game has ended
- **Automatic Dumps**: Game state is automatically dumped when BINGO is reached, and incomplete games (no BINGO) are dumped on newgame
- Contains full game history and state for analysis, debugging, and audit purposes
- JSON structure includes creation timestamp, end timestamp, and all game components
- Files use pretty-printed JSON for human readability
- Game dumps are append-only (no overwriting of existing files)

#### GET /runninggameid

Get the current running game ID and creation details.

**Authentication Required:** No

**Request:**
```bash
curl http://127.0.0.1:3000/runninggameid
```

**Success Response (200 OK):**
```json
{
  "game_id": "game_87654321",
  "created_at": "2025-07-17 14:35:12 UTC",
  "created_at_timestamp": {
    "secs_since_epoch": 1752767712,
    "nanos_since_epoch": 345123000
  }
}
```

**Notes:**
- Returns the unique identifier and creation details of the currently running game instance
- `game_id`: 8-digit hexadecimal identifier for the current game (format: `game_12345678`)
- `created_at`: Human-readable timestamp in UTC format
- `created_at_timestamp`: SystemTime representation with seconds and nanoseconds since Unix epoch
- Available to all clients without authentication
- Useful for tracking game instances, logging, and client synchronization
- Game ID changes when a new game is started via `/newgame` endpoint

## Card Structure

Tombola cards follow specific rules:

1. **Grid Structure**: 3 rows × 9 columns
2. **Number Distribution**: Each card contains exactly 15 numbers and 12 empty cells
3. **Column Ranges**:
   - Column 1: 1-9
   - Column 2: 10-19
   - Column 3: 20-29
   - Column 4: 30-39
   - Column 5: 40-49
   - Column 6: 50-59
   - Column 7: 60-69
   - Column 8: 70-79
   - Column 9: 80-90
4. **Row Constraints**: Each row contains exactly 5 numbers and 4 empty cells
5. **Anti-adjacency**: When generating card groups, numbers are distributed to prevent adjacent duplicates

## Example Usage

### Complete Client Workflow

**Important**: Client registration must be completed before any numbers are extracted from the pouch.

1. **Register a client with default card (1 card):**
```bash
curl -X POST http://127.0.0.1:3000/register \
  -H "Content-Type: application/json" \
  -d '{"name": "player1", "client_type": "player"}'
```

2. **Register a client with specific number of cards:**
```bash
curl -X POST http://127.0.0.1:3000/register \
  -H "Content-Type: application/json" \
  -d '{"name": "player2", "client_type": "player", "nocard": 6}'
```

**Note**: After the first number is extracted via `/extract`, registration will fail:
```bash
# This will return 409 Conflict if numbers have been extracted
curl -X POST http://127.0.0.1:3000/register \
  -H "Content-Type: application/json" \
  -d '{"name": "lateplayer", "client_type": "player"}'
```

3. **Get client information by name:**
```bash
curl http://127.0.0.1:3000/client/player1
```

3a. **Get client information by ID:**
```bash
curl http://127.0.0.1:3000/clientbyid/A1B2C3D4E5F6G7H8
```

4. **List assigned cards:**
```bash
curl http://127.0.0.1:3000/listassignedcards \
  -H "X-Client-ID: A1B2C3D4E5F6G7H8"
```

5. **Get specific card:**
```bash
curl http://127.0.0.1:3000/getassignedcard/card_id_1 \
  -H "X-Client-ID: A1B2C3D4E5F6G7H8"
```

5. **Check game state:**
```bash
curl http://127.0.0.1:3000/status
curl http://127.0.0.1:3000/runninggameid
curl http://127.0.0.1:3000/board
curl http://127.0.0.1:3000/pouch
curl http://127.0.0.1:3000/scoremap
```

## Rate Limiting

Currently, no rate limiting is implemented. The server uses a connection timeout of 100ms for accepting new connections.

## Concurrency

The server supports concurrent connections and uses Arc<Mutex<>> for thread-safe access to shared state:
- Board state
- Pouch state
- Client registry
- Card assignments

## Server Configuration

- **Host**: 127.0.0.1 (localhost only)
- **Port**: 3000
- **Protocol**: HTTP/1.1
- **Runtime**: Tokio async runtime
- **HTTP Library**: Hyper with hyper-util

## Shutdown

The server supports graceful shutdown via an atomic boolean signal. When shutdown is requested, the server will:
1. Stop accepting new connections
2. Allow existing connections to complete
3. Print "API Server shutting down..." message

# Tombola API Documentation

## Overview

The Tombola API provides a RESTful HTTP interface for managing multiple concurrent tombola games, client registration, and card management. The server runs on `http://127.0.0.1:3000` and uses JSON for request/response payloads.

## Multi-Game Architecture

The Tombola server uses a **GameRegistry** system that supports multiple concurrent games with complete isolation:

- **Game-Specific Routing**: All game operations use `/{game_id}/` routing pattern for complete isolation
- **Unique Game IDs**: Each game instance has a randomly generated 8-digit hexadecimal identifier (format: `game_12345678`)
- **Creation Timestamps**: Games include creation timestamps for tracking and debugging purposes
- **Thread-Safe Components**: All game state is managed through coordinated `Arc<Mutex<T>>` access per game
- **State Components**: Board, Pouch, ScoreCard, ClientRegistry, and CardAssignmentManager per game
- **Enhanced Responses**: Game ID and creation time are included in status and management API responses
- **Game Management**: Global endpoints for game creation (`/newgame`) and discovery (`/gameslist`)

## Base URL
```
http://127.0.0.1:3000
```

## Routing Index

### Global Endpoints (No Game ID)
| Method | Endpoint | Description | Auth Required |
|--------|----------|-------------|---------------|
| `POST` | `/newgame` | Create new game | Board Client |
| `GET` | `/gameslist` | List all available games | None |
| `POST` | `/register` | Register client globally (without joining game) | None |
| `GET` | `/clientinfo` | Get client information by name (query param) | None |
| `GET` | `/clientinfo/{client_id}` | Get client information by ID | None |

### Game-Specific Endpoints
| Method | Endpoint | Description | Auth Required |
|--------|----------|-------------|---------------|
| `POST` | `/{game_id}/join` | Join client to specific game | None |
| `POST` | `/{game_id}/generatecards` | Generate cards for client in game | Client ID |
| `GET` | `/{game_id}/listassignedcards` | List assigned cards for client | Client ID |
| `GET` | `/{game_id}/getassignedcard/{card_id}` | Get specific card by ID | Client ID |
| `GET` | `/{game_id}/board` | Get extracted numbers for game | None |
| `GET` | `/{game_id}/pouch` | Get remaining numbers for game | None |
| `GET` | `/{game_id}/status` | Get overall status for game | None |
| `GET` | `/{game_id}/players` | Get list of players and their card counts | Client ID |
| `GET` | `/{game_id}/scoremap` | Get scores and achievements for game | None |
| `POST` | `/{game_id}/extract` | Extract next number in game | Board Client |
| `POST` | `/{game_id}/dumpgame` | Dump specific game state to JSON | Board Client |

**Authentication Notes:**
- **None**: No authentication required
- **Client ID**: Requires valid client ID in `X-Client-ID` header (client must be registered to the game)
- **Board Client**: Requires client ID in `X-Client-ID` header AND client must have client_type "board"

## Authentication

Client authentication is required for most game-specific endpoints via the `X-Client-ID` header. All clients (including board clients) must register to specific games first to obtain access.

### Client Types and Registration
- **Board Clients**: Must register with `client_type: "board"` to extract numbers and manage games
  - Receive special BOARD_ID card (`0000000000000000`) during registration
  - Only board clients can extract numbers and create new games
- **Player Clients**: Register with `client_type: "player"` for card management and gameplay
  - Must register separately to each game they want to join
  - Receive regular numbered cards during registration

### Authorization Levels
- **Game Registration Required**: Client must be registered to the specific game via `/{game_id}/join`
- **Board Client Only**: Endpoint restricted to clients with `client_type: "board"`

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

## Endpoints

### 1. Game Management

#### POST /newgame

Create a new game instance in the GameRegistry. This endpoint does not destroy existing games but creates a new isolated game.

**Authentication Required:** Board Client (registered client with client_type "board")

**Success Response (200 OK):**
```json
{
  "message": "New game created",
  "game_id": "game_12345678",
  "created_at": "2025-07-22 08:51:49 UTC"
}
```

**Notes:**
- **Multi-Game Behavior**: Creates a completely new game instance without affecting existing games
- Only the board client can create new games
- Returns unique game ID for the new game instance
- New game is immediately available for client registration
- Existing games continue to operate independently

#### GET /gameslist

List all available games with their current status and statistics.

**No Authentication Required**

**Success Response (200 OK):**
```json
{
  "games": [
    {
      "game_id": "game_12345678",
      "status": "New",
      "created_at": "2025-07-22 08:51:49 UTC",
      "client_count": 0,
      "extracted_numbers": 0,
      "owner": "BOARD_CLIENT_ID"
    },
    {
      "game_id": "game_87654321",
      "status": "Active",
      "created_at": "2025-07-22 08:45:12 UTC",
      "client_count": 3,
      "extracted_numbers": 15,
      "owner": "ANOTHER_BOARD_ID"
    }
  ]
}
```

**Notes:**
- Used by smart clients for automatic game discovery
- Shows all games regardless of their state
- Includes game statistics for informed decision making
- `owner` field shows the ClientID of the board client that created each game

### 2. Global Client Registration

#### POST /register

Register a new client globally without joining a specific game. This creates a client account that can later join multiple games.

**No Authentication Required**

**Request Body:**
```json
{
  "name": "client_name",
  "client_type": "player|board|admin"
}
```

**Success Response (200 OK):**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "message": "Client 'client_name' registered successfully globally"
}
```

**Success Response - Existing Client (200 OK):**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "message": "Client 'client_name' already registered globally"
}
```

**Error Response - Server Error (500 Internal Server Error):**
```json
{
  "error": "Failed to register client globally"
}
```

**Notes:**
- **Global Registration**: Creates a client account that can be used across multiple games
- **Reusable Client ID**: The same client ID can be used to join multiple games
- **No Game Association**: This endpoint does not associate the client with any specific game
- **Duplicate Handling**: If a client with the same name already exists, returns the existing client information
- **Future Game Joining**: After global registration, use `/{game_id}/join` to join specific games
- **Client ID Persistence**: The client ID remains the same across all games the client joins

### 3. Game-Specific Client Registration

#### POST /{game_id}/join

Join a client to a specific game (registers if needed).

**Path Parameters:**
- `game_id`: ID of the game to join (e.g., `game_12345678`)

**Request Body:**
```json
{
  "name": "client_name",
  "client_type": "player|board",
  "nocard": 6,  // Optional: number of cards to generate during registration (default: 1)
  "email": "optional@email.com"  // Optional: email address for the client
}
```

**Client Type Details:**
- `"player"`: Standard player client, receives regular numbered cards
- `"board"`: Board client, receives special BOARD_ID card, can extract numbers

**Card Assignment:**
- **Player clients**: Receive regular cards with unique IDs containing random numbers
- **Board clients**: Receive exactly one card with ID `"0000000000000000"` representing the entire game board

**Success Response (200 OK):**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "message": "Client 'client_name' joined game game_12345678 successfully"
}
```

**Error Response - Join After Game Started (409 Conflict):**
```json
{
  "error": "Cannot register new clients after numbers have been extracted in this game"
}
```

**Error Response - Game Not Found (404 Not Found):**
```json
{
  "error": "Game game_12345678 not found"
}
```

**Notes:**
- **Game Isolation**: Joining is specific to the game ID in the path
- **Game State Restriction**: New clients can only join when no numbers have been extracted from the pouch in this specific game
- Once the first number is extracted in a game, all new join attempts to that game will fail with a 409 Conflict error
- This ensures fair play by preventing players from joining mid-game
- **Client-Side Card Optimization**: Smart clients will first join without requesting cards, then check if cards are already assigned before generating new ones
- If `nocard` is not specified, the server will automatically generate 1 card for the client by default
- If `nocard` is specified, the server will generate and assign the requested number of cards to the client
- If client already exists in this game, returns existing client information
- Client ID is generated using a hash of name, type, and timestamp

### 4. Client Information (Global)

#### GET /clientinfo

Retrieve information about a registered client by name across all games.

**Query Parameters:**
- `name`: Name of the client to retrieve

**Example Request:**
```
GET /clientinfo?name=player1
```

**Response:**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "name": "client_name",
  "client_type": "player",
  "registered_at": "SystemTime representation"
}
```

#### GET /clientinfo/{client_id}

Retrieve information about a registered client by client ID across all games.

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
- These are **global** routes that search across all games in the GameRegistry
- Returns information about clients regardless of which specific game they're registered to
- Client data can span multiple games - same client can exist in multiple games
- **Global Client IDs**: A client maintains the same ID across all games they join
- The server uses a global client registry to ensure ID consistency
- When a client registers to multiple games, they reuse their existing global client ID

### 5. Card Management (Game-Specific)

#### POST /{game_id}/generatecards

Generate cards for a registered client within a specific game.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

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
  "message": "Generated 6 cards successfully for game game_12345678"
}
```

**Notes:**
- Card generation is specific to the game and client
- Only allowed during registration or if client has no existing cards in this game
- Each card is a 3x9 grid following tombola rules
- `null` represents empty cells in the card
- Cards are generated in groups of 6 with anti-adjacency patterns
- Cards are isolated per game - same client can have different cards in different games

#### GET /{game_id}/listassignedcards

List all cards assigned to a client within a specific game.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

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

#### GET /{game_id}/getassignedcard/{card_id}

Retrieve a specific card assigned to a client within a specific game.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)
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

### 6. Board & Game State (Game-Specific)

#### GET /{game_id}/board

Get the current board state (extracted numbers) for a specific game.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

**Response:**
```json
{
  "numbers": [15, 23, 37, 41, 52, 68, 74, 89],
  "marked_numbers": [15, 23, 37, 41, 52, 68, 74, 89]
}
```

**Notes:**
- Returns Board struct with numbers array (in extraction order) and marked_numbers set for specific game
- Empty arrays if no numbers have been extracted yet in this game
- Board state is completely isolated per game

#### GET /{game_id}/pouch

Get the current pouch state (remaining numbers) for a specific game.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

**Response:**
```json
{
  "numbers": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 38, 39, 40, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 69, 70, 71, 72, 73, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 90]
}
```

**Notes:**
- Returns Pouch struct directly with numbers array for specific game
- `numbers` contains all numbers that haven't been extracted yet in this game
- The count of remaining numbers can be obtained from the length of the numbers array
- Pouch state is completely isolated per game

#### GET /{game_id}/scoremap

Get the current scorecard and score map (prize tracking information) for a specific game.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

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
- Returns ScoreCard struct with `published_score` and `score_map` fields for specific game
- `published_score`: The highest score achieved so far in this game (current published achievement level)
- `score_map`: Map of score indices to arrays of ScoreAchievement objects for this specific game
- Each ScoreAchievement contains:
  - `client_id`: The ID of the client who achieved the score (or "0000000000000000" for board achievements)
  - `card_id`: The ID of the card that achieved the score (or "0000000000000000" for board achievements)
  - `numbers`: Array of specific numbers that contributed to achieving that score level in this game
- Returns `published_score: 0` if no achievements have been recorded yet in this game
- Each key in score_map represents a score level:
  - `2`, `3`, `4`, `5`: Number of numbers in a line achievement
  - `15`: BINGO (full card completion)
- For line achievements, `numbers` contains only the numbers from the winning line
- For BINGO achievements, `numbers` contains all 15 numbers that completed the card
- Empty score_map `{}` if no scores have been recorded yet in this game
- Score data is completely isolated per game

#### GET /{game_id}/status

Get overall server status and specific game information.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

**Response:**
```json
{
  "status": "new",
  "game_id": "game_12345678",
  "created_at": "2025-07-17 14:30:25 UTC",
  "owner": "BOARD_CLIENT_ID",
  "players": "4",
  "cards": "20",
  "numbers_extracted": 8,
  "scorecard": 5
}
```

**Response (for closed game):**
```json
{
  "status": "closed",
  "game_id": "game_12345678",
  "created_at": "2025-07-17 14:30:25 UTC",
  "closed_at": "2025-07-17 15:45:10 UTC",
  "owner": "BOARD_CLIENT_ID",
  "players": "4",
  "cards": "20",
  "numbers_extracted": 45,
  "scorecard": 15
}
```

**Notes:**
- `status`: Current game state - one of "new", "active", or "closed"
  - `new`: No numbers have been extracted yet
  - `active`: At least one number has been extracted but BINGO hasn't been reached
  - `closed`: BINGO has been reached (scorecard = 15)
- `game_id`: Unique 8-digit hexadecimal identifier for the specific game
- `created_at`: Human-readable timestamp when this specific game was created
- `owner`: Client ID of the board client that created this game
- `closed_at`: Human-readable timestamp when the game was closed (only present if status is "closed")
- `players`: Number of registered players in this game (as string)
- `cards`: Total number of cards assigned in this game (as string)
- `numbers_extracted`: Total count of numbers extracted so far in this game
- `scorecard`: Current published score (highest achievement level reached) in this game

#### GET /{game_id}/players

Get a detailed list of all players (clients) registered to a specific game, including their client types and card counts.

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

**Authentication Required:** `X-Client-ID` header with a registered client ID for the specified game

**Success Response (200 OK):**
```json
{
  "game_id": "game_12345678",
  "total_players": 4,
  "total_cards": 24,
  "players": [
    {
      "client_id": "BOARD_CLIENT_ID",
      "client_type": "board",
      "card_count": 1
    },
    {
      "client_id": "A1B2C3D4E5F6G7H8",
      "client_type": "player",
      "card_count": 6
    },
    {
      "client_id": "B2C3D4E5F6G7H8I9",
      "client_type": "player",
      "card_count": 12
    },
    {
      "client_id": "C3D4E5F6G7H8I9J0",
      "client_type": "player",
      "card_count": 5
    }
  ]
}
```

**Error Responses:**

**400 Bad Request - Missing Authentication:**
```json
{
  "error": "Client ID header (X-Client-ID) is required"
}
```

**403 Forbidden - Not Registered:**
```json
{
  "error": "Client 'CLIENT_ID' is not registered in game 'GAME_ID'"
}
```

**404 Not Found - Game Not Found:**
```json
{
  "error": "Game 'game_12345678' not found"
}
```

**Notes:**
- `game_id`: Unique 8-digit hexadecimal identifier for the specific game
- `total_players`: Total number of clients registered to this game
- `total_cards`: Sum of all cards assigned to all players in this game
- `players`: Array of player objects, sorted by client type (board clients first) then by client ID
  - `client_id`: Unique identifier for the client
  - `client_type`: Type of client ("board", "player", etc.)
  - `card_count`: Number of cards assigned to this specific client in this game
- Board clients show 0 cards (BOARD_ID cards are excluded from player card counts)
- Player clients can have multiple cards based on their requests
- Authentication required: Only registered clients (board or player) can access this endpoint
- Useful for game monitoring, statistics, and administrative purposes

#### POST /{game_id}/extract

Extract the next number from the pouch for a specific game (remote extraction control).

**Path Parameters:**
- `game_id`: ID of the game (e.g., `game_12345678`)

**Authentication Required:** Yes (X-Client-ID header required)

**Authorization:** Only registered board clients (client_type "board") can extract numbers.

**Request:**
```bash
curl -X POST http://127.0.0.1:3000/game_12345678/extract
  -H "X-Client-ID: <board_client_id>"
  -H "Content-Type: application/json"
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "extracted_number": 42,
  "numbers_remaining": 82,
  "total_extracted": 8,
  "message": "Number 42 extracted successfully from game game_12345678"
}
```

**Error Response - Unauthorized Client (403 Forbidden):**
```json
{
  "error": "Unauthorized: Only board client can extract numbers"
}
```

**Error Response - Game Not Found (404 Not Found):**
```json
{
  "error": "Game game_12345678 not found"
}
```

**Error Response - Pouch Empty (409 Conflict):**
```json
{
  "error": "No numbers remaining in pouch for game game_12345678"
}
```

**Error Response - Authentication (400 Bad Request):**
```json
{
  "error": "Client ID header (X-Client-ID) is required"
}
```

**Notes:**
- Performs extraction logic for the specific game only
- **Security**: Only registered board clients (client_type "board") are authorized to extract numbers
- Regular game clients cannot trigger extractions for security and game integrity
- Automatically updates the board state, scorecard, and marked numbers for the specific game
- Follows the coordinated mutex locking pattern to ensure thread safety per game
- Returns detailed information about the extraction result for the specific game
- `numbers_remaining`: Count of numbers still available in the pouch for this game
- `total_extracted`: Total numbers extracted so far (including this one)
- Server logs the extraction with client identification for audit purposes

#### POST /newgame

**COMPLETE GAME RESET** - Destroys all game state and persistent data to start a completely fresh game.

**IMPORTANT**: This is a **destructive operation** that:
- Forces ALL clients to re-register (all client sessions destroyed)
- Destroys ALL card assignments (clients must get new cards)
- Resets ALL game progress (board, pouch, scores completely recreated)
- Generates a new unique game ID
- Creates a fresh game timestamp

**Authentication Required:** Yes (X-Client-ID header required)

**Authorization:** Only registered board clients (client_type "board") can reset the game.

**Request:**
```bash
curl -X POST http://127.0.0.1:3000/newgame \
  -H "X-Client-ID: <board_client_id>" \
  -H "Content-Type: application/json"
```

**Success Response (200 OK):**
```json
{
  "success": true,
  "message": "Game reset",
  "game_id": "game_87654321",
  "created_at": "2025-07-17 14:35:12 UTC"
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
- **Security**: Only registered board clients (client_type "board") are authorized to reset the game
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

### Complete Multi-Game Client Workflow

**Important**: Client registration must be completed before any numbers are extracted from the pouch in each specific game.

1. **Discover available games:**
```bash
curl http://127.0.0.1:3000/gameslist
```

2. **Create a new game (board client only):**
```bash
curl -X POST http://127.0.0.1:3000/newgame \
  -H "X-Client-ID: <board_client_id>" \
  -H "Content-Type: application/json"
```

3. **Register a client globally (optional - creates reusable client account):**
```bash
curl -X POST http://127.0.0.1:3000/register \
  -H "Content-Type: application/json" \
  -d '{"name": "player1", "client_type": "player"}'
```

4. **Join a client to a specific game with default card (1 card):**
```bash
curl -X POST http://127.0.0.1:3000/game_12345678/join \
  -H "Content-Type: application/json" \
  -d '{"name": "player1", "client_type": "player"}'
```

5. **Join a client to a specific game with multiple cards:**
```bash
curl -X POST http://127.0.0.1:3000/game_12345678/join \
  -H "Content-Type: application/json" \
  -d '{"name": "player2", "client_type": "player", "nocard": 6}'
```

**Note**: After the first number is extracted via `/{game_id}/extract`, joining will fail for that specific game:
```bash
# This will return 409 Conflict if numbers have been extracted in game_12345678
curl -X POST http://127.0.0.1:3000/game_12345678/join \
  -H "Content-Type: application/json" \
  -d '{"name": "lateplayer", "client_type": "player"}'
```

6. **Get client information by name (global search):**
```bash
curl "http://127.0.0.1:3000/clientinfo?name=player1"
```

7. **Get client information by ID (global search):**
```bash
curl http://127.0.0.1:3000/clientinfo/A1B2C3D4E5F6G7H8
```

8. **List assigned cards in specific game:**
```bash
curl http://127.0.0.1:3000/game_12345678/listassignedcards \
  -H "X-Client-ID: A1B2C3D4E5F6G7H8"
```

9. **Get specific card in specific game:**
```bash
curl http://127.0.0.1:3000/game_12345678/getassignedcard/card_id_1 \
  -H "X-Client-ID: A1B2C3D4E5F6G7H8"
```

10. **Check game state for specific game:**
```bash
curl http://127.0.0.1:3000/game_12345678/status
curl http://127.0.0.1:3000/game_12345678/board
curl http://127.0.0.1:3000/game_12345678/pouch
curl http://127.0.0.1:3000/game_12345678/scoremap
```

11. **Extract numbers in specific game (board client only):**
```bash
curl -X POST http://127.0.0.1:3000/game_12345678/extract \
  -H "X-Client-ID: <board_client_id>"
```

12. **Dump specific game state:**
```bash
curl -X POST http://127.0.0.1:3000/game_12345678/dumpgame \
  -H "X-Client-ID: <board_client_id>"
```

## Rate Limiting

Currently, no rate limiting is implemented. The server uses a connection timeout of 100ms for accepting new connections.

## Concurrency

The server supports concurrent connections and uses Arc<Mutex<>> for thread-safe access to shared state:
- **GameRegistry**: Thread-safe access to multiple games
- **Per-Game State**: Board state, Pouch state, Client registry, Card assignments (all per game)
- **Game Isolation**: Complete separation of game state between different games

## Server Configuration

- **Host**: 127.0.0.1 (localhost only)
- **Port**: 3000
- **Protocol**: HTTP/1.1
- **Runtime**: Tokio async runtime
- **HTTP Library**: Axum web framework

## Shutdown

The server supports graceful shutdown via an atomic boolean signal. When shutdown is requested, the server will:
1. Stop accepting new connections
2. Allow existing connections to complete
3. Print "API Server shutting down..." message

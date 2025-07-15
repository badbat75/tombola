# Tombola API Documentation

## Overview

The Tombola API provides a RESTful HTTP interface for managing tombola games, client registration, and card management. The server runs on `http://127.0.0.1:3000` and uses JSON for request/response payloads.

## Base URL
```
http://127.0.0.1:30005. **Get specific card:**
```bash
curl http://127.0.0.1:3000/getassignedcard/card_id_1 \
  -H "X-Client-ID: A1B2C3D4E5F6G7H8"
```

6. **Check game state:**
## Authentication

Most endpoints require client authentication via the `X-Client-ID` header. Clients must register first to obtain a client ID.

## Common Headers

- `Content-Type: application/json` (for POST requests)
- `X-Client-ID: <client_id>` (for authenticated endpoints)
- `Access-Control-Allow-Origin: *` (included in all responses)

## Data Types

- `Number`: 8-bit unsigned integer (u8) representing tombola numbers (1-90)
- `Card`: 3x9 grid where each cell can contain a number or be empty (represented as `Option<u8>`)

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

**Response:**
```json
{
  "client_id": "A1B2C3D4E5F6G7H8",
  "message": "Client 'client_name' registered successfully"
}
```

**Notes:**
- If `nocard` is not specified, the server will automatically generate 1 card for the client by default
- If `nocard` is specified, the server will generate and assign the requested number of cards to the client
- If client already exists, returns existing client information
- Client ID is generated using a hash of name, type, and timestamp

### 2. Client Information

#### GET /client/{client_name}

Retrieve information about a registered client.

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
  "board": [15, 23, 37, 41, 52, 68, 74, 89]
}
```

**Notes:**
- Returns array of numbers in the order they were extracted
- Empty array if no numbers have been extracted yet

#### GET /pouch

Get the current pouch state (remaining numbers).

**Response:**
```json
{
  "pouch": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 38, 39, 40, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 69, 70, 71, 72, 73, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 90],
  "remaining": 82
}
```

**Notes:**
- `pouch` contains all numbers that haven't been extracted yet
- `remaining` is the count of numbers still in the pouch

#### GET /scorecard

Get the current scorecard (last extracted number).

**Response:**
```json
{
  "scorecard": 74
}
```

**Notes:**
- Returns the most recently extracted number
- Returns `0` if no numbers have been extracted yet

#### GET /status

Get overall server status and game information.

**Response:**
```json
{
  "status": "running",
  "numbers_extracted": 8,
  "scorecard": 74,
  "server": "tokio-hyper"
}
```

**Notes:**
- `numbers_extracted`: Total count of numbers extracted so far
- `scorecard`: Last extracted number
- `server`: Server implementation identifier

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

3. **Get client information:**
```bash
curl http://127.0.0.1:3000/client/player1
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
curl http://127.0.0.1:3000/board
curl http://127.0.0.1:3000/pouch
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

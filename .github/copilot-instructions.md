# Tombola Game - AI Coding Assistant Instructions

## Architecture Overview

This is a Rust-based multi-binary tombola/bingo game with a client-server architecture:

- **`tombola-server`** (`src/tombola_server.rs`): Main game server with terminal UI and HTTP API
- **`board_client`** (`src/board_client.rs`): Terminal client that displays current game state
- **`card_client`** (`src/card_client.rs`): Interactive client for card management and gameplay

## Core Components & Data Flow

### Shared State Management
The server uses `Arc<Mutex<T>>` for thread-safe shared state:
- `Board`: Tracks extracted numbers and marked positions
- `Pouch`: Contains remaining numbers to extract (1-90)
- `ScoreCard`: Manages scoring and prize tracking
- `CardAssignmentManager`: Handles client card assignments

### Critical Mutex Coordination Pattern
**Always acquire locks in consistent order** to prevent deadlocks:
```rust
// CORRECT: Acquire locks in this order
if let Ok(pouch) = pouch_ref.lock() {
    if let Ok(board) = board_ref.lock() {
        if let Ok(scorecard) = scorecard_ref.lock() {
            // Perform coordinated operations
        }
    }
}
```

### Game Configuration (`src/defs.rs`)
- `BOARDCONFIG`: Defines card layout (5×3 numbers, 2×3 card grid)
- `NUMBERSPERCARD`: 15 numbers per card (5 columns × 3 rows)
- Numbers range: 1-90 (calculated as `FIRSTNUMBER` to `LASTNUMBER`)

## Key Development Patterns

### HTTP API Server (`src/server.rs`)
- Hyper-based async server on `127.0.0.1:3000`
- All endpoints return JSON with CORS headers
- Client authentication via `X-Client-ID` header
- Error responses use standard HTTP status codes

### Card Generation Algorithm (`src/card.rs`)
Cards are generated as groups of 6 with anti-adjacency rules:
- Each card has 15 numbers distributed across 9 columns (1-10, 11-20, ..., 81-90)
- Numbers are positioned to avoid adjacent placement across cards
- Use `CardManagement::generate_card_group()` for compliant card sets

### Terminal UI (`src/terminal.rs`)
- Uses crossterm for cross-platform terminal control
- Color coding: Green for current number, Yellow for marked/winning numbers
- Board layout calculated with `downrightshift()` for proper spacing

## Build & Run Commands

```bash
# Build all binaries
cargo build --release

# Run main server (includes terminal UI)
cargo run --bin tombola-server

# Run display-only client
cargo run --bin board_client

# Run interactive card client
cargo run --bin card_client
```

## API Integration Patterns

### Client Registration Flow
1. POST `/register` with `{name, client_type, nocard}`
2. Store returned `client_id` for subsequent requests
3. Use `X-Client-ID` header for authenticated endpoints

### Real-time Game State
- GET `/board` - Current extracted numbers
- GET `/pouch` - Remaining numbers count
- GET `/scoremap` - Current scores and winners with score map

### Card Management
- POST `/generatecards` - Generate new card sets
- POST `/assigncard` - Assign cards to clients
- GET `/listassignedcards` - View all assignments

## Testing & Debugging

- Server logs to stdout with connection and error details
- Use `docs/TOMBOLA_API.md` for complete API reference
- Test API endpoints with curl using examples in documentation
- Terminal clients provide immediate visual feedback for server state

## File Organization

- `src/defs.rs`: Core constants and type definitions
- `src/board.rs`: Game board state management
- `src/score.rs`: Scoring logic and prize calculations
- `src/card.rs`: Card generation and assignment logic
- `src/client.rs`: Client registration and management
- `src/server.rs`: HTTP API server implementation
- `src/terminal.rs`: Terminal UI rendering
- `src/pouch.rs`: Number extraction logic

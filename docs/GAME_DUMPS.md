# Game State Dumps

This feature automatically dumps the complete game state to JSON files when games end.

## When Game Dumps Occur

1. **Automatic Dump on BINGO**: When any player reaches BINGO (15 numbers matched), the game state is automatically dumped to `data/games/`
2. **Selective Dump on New Game**: When starting a new game via `/newgame` endpoint, only incomplete games (started but no BINGO reached) are dumped before reset. This avoids duplicate dumps since BINGO games are already saved when BINGO occurs.
3. **Manual Dump**: Using the `/dumpgame` endpoint (requires board client authentication) - dumps regardless of game state

## File Location and Naming

- **Directory**: `data/games/`
- **Filename Format**: `{game_id}.json`
- **Example**: `game_1a2b3c4d.json`

## JSON Structure

The dumped JSON contains:
- `id`: Unique game identifier
- `created_at`: Game creation timestamp
- `game_ended_at`: Game end timestamp  
- `board`: All extracted numbers and marked positions
- `pouch`: Remaining numbers available for extraction
- `scorecard`: Current scores and achievements
- `client_registry`: All registered clients and their information
- `card_manager`: All card assignments and card data

## API Endpoints

### Manual Dump
```bash
curl -X POST http://127.0.0.1:3000/dumpgame \
  -H "X-Client-ID: 0000000000000000"
```

### New Game (Auto-dumps current game)
```bash
curl -X POST http://127.0.0.1:3000/newgame \
  -H "X-Client-ID: 0000000000000000"
```

## Security

- Only the board client (ID: `0000000000000000`) can trigger manual dumps
- All game dumps include complete game state for analysis and auditing
- Dumps are created in append-only fashion (no overwriting)

## Use Cases

1. **Game Analysis**: Review game progression and winning patterns
2. **Audit Trail**: Maintain records of all completed games
3. **Bug Investigation**: Capture game state for debugging issues
4. **Statistics**: Analyze game duration, card distributions, etc.

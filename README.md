# Tombola Game

A Rust-based tombola (bingo) game with visual board display and scoring system.

## Features

- Visual board display with proper spacing
- Real-time score checking (2, 3, 4, 5 in a row, BINGO)
- Interactive gameplay with keyboard controls
- Color-coded display for extracted numbers
- Configurable board layout

## Configuration

The game uses a 2×3 grid of cards (6 cards total), each containing 5×3 numbers (15 numbers per card).

## Controls

- **Any key**: Draw next number
- **ESC**: Exit game

## Build and Run

```bash
cargo build --release
cargo run
```

## Dependencies

- `crossterm` - For terminal manipulation and keyboard input
- `rand` - For random number generation
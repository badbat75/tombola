// src/defs.rs
// This module defines the basic structures and constants used in the Tombola game.

// Type alias for numbers used in the Tombola game
pub type Number = u8;

pub struct BoardStruct {
    pub cols_per_card: Number,
    pub rows_per_card: Number,
    pub cards_per_row: Number,
    pub cards_per_col: Number,
    pub hnumbers_space: Number,
    pub vnumbers_space: Number,
    pub hcards_space: Number,
    pub vcards_space: Number,
}

pub const BOARDCONFIG: BoardStruct = BoardStruct {
    cols_per_card: 5, // number of columns in a card
    rows_per_card: 3, // number of rows in a card
    cards_per_row: 2, // number of cards in a row
    cards_per_col: 3, // number of cards in a column
    hnumbers_space: 2, // space between numbers in the same row
    vnumbers_space: 1, // space between numbers in the same column
    hcards_space: 2, // space between cards in the same row
    vcards_space: 1, // space between cards in the same column
};

pub const FIRSTNUMBER: Number = 1;
pub const LASTNUMBER: Number = BOARDCONFIG.cols_per_card * BOARDCONFIG.rows_per_card * BOARDCONFIG.cards_per_row * BOARDCONFIG.cards_per_col - 1 + FIRSTNUMBER;
pub const NUMBERSPERCARD: Number = BOARDCONFIG.cols_per_card * BOARDCONFIG.rows_per_card;
pub const CARDSNUMBER: Number = BOARDCONFIG.cards_per_row * BOARDCONFIG.cards_per_col;

// Color definitions for terminal output (ESC sequences)
pub struct Colors;

#[allow(dead_code)]
impl Colors {
    #[must_use] pub fn green() -> &'static str {
        "\x1b[1;32m" // Bold Green - for current/last number
    }

    #[must_use] pub fn yellow() -> &'static str {
        "\x1b[1;33m" // Bold Yellow - for marked numbers and prizes
    }

    #[must_use] pub fn reset() -> &'static str {
        "\x1b[0m" // Reset - to reset formatting
    }

    #[must_use] pub fn red() -> &'static str {
        "\x1b[1;31m" // Bold Red - for errors or warnings
    }

    #[must_use] pub fn blue() -> &'static str {
        "\x1b[1;34m" // Bold Blue - for information
    }

    #[must_use] pub fn magenta() -> &'static str {
        "\x1b[1;35m" // Bold Magenta - for special highlights
    }
}

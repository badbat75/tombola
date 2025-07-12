pub struct BoardStruct {
    pub cols_per_card: u8,
    pub rows_per_card: u8,
    pub cards_per_row: u8,
    pub cards_per_col: u8,
    pub hnumbers_space: u8,
    pub vnumbers_space: u8,
    pub hcards_space: u8,
    pub vcards_space: u8,
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

#[derive(Clone)]
pub struct NumberEntry {
    pub number: u8,
    pub is_marked: bool,
}

pub const FIRSTNUMBER: u8 = 1;
pub const LASTNUMBER: u8 = BOARDCONFIG.cols_per_card * BOARDCONFIG.rows_per_card * BOARDCONFIG.cards_per_row * BOARDCONFIG.cards_per_col - 1 + FIRSTNUMBER;
pub const NUMBERSPERCARD: u8 = BOARDCONFIG.cols_per_card * BOARDCONFIG.rows_per_card;

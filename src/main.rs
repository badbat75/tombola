// src/main.rs
// This is the main entry point for the Tombola game.

mod defs;
use defs::*;
mod prize;
mod terminal;

enum IOList {
    Terminal,
}

// Default program input/output:
const IO: IOList = IOList::Terminal;

fn next_extraction (iodevice: IOList) -> bool {
    match iodevice {
        IOList::Terminal => { terminal::hitkey() }
    }
}

fn show_on(iodevice: IOList, board: &[NumberEntry], pouch: &[u8], extracted: u8, scorecard: &mut u8, itemsleft: usize) {
    match iodevice {
        IOList::Terminal => { terminal::show_on_terminal(board, pouch, extracted, scorecard, itemsleft) }
    }
}

// Function to wait for a key press and return true if ESC is pressed, false otherwise
fn main() {
    let mut pouch: Vec<u8> = (FIRSTNUMBER..=LASTNUMBER).collect();
    let mut board: Vec<NumberEntry> = Vec::new();
    let mut itemsleft = pouch.len();
    let mut scorecard = 0;

    while ! pouch.is_empty() {
        // Expect event for next extraction
        match next_extraction(IO) {
            true => {
                break;
            }
            false => {}
        }
        // Randomly extract a number index from the pouch
        let random_index = rand::random_range(0..itemsleft);
        let extracted = pouch.remove(random_index);
        // Add the extracted number to the board
        board.push(NumberEntry {
            number: extracted,
            is_marked: false,
        });
        itemsleft = pouch.len();
        // Check the score based on the current board and extracted number
        prize::tombola_prize_check(&mut board, extracted, &mut scorecard);
        // Show the current state on configured IO device
        show_on(IO, &board, &pouch, extracted, &mut scorecard, itemsleft);

        // If the scorecard reaches the number of numbers per card, break the loop
        if scorecard == NUMBERSPERCARD { break };
    }
}
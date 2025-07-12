// tombola/src/terminal.rs
// This module handles terminal input/output for the Tombola game.

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use crate::defs::{NumberEntry, BOARDCONFIG, NUMBERSPERCARD};

pub struct DeltaPos {
    pub delta_x: u8,
    pub delta_y: u8,
}

// Function to calculate the horizontal and vertical shifts
pub fn downrightshift(prev_num: u8, curr_num: u8) -> DeltaPos {
    let prev_num = prev_num as i8;
    let curr_num = curr_num as i8;
    let numbers_per_row = (BOARDCONFIG.cols_per_card * BOARDCONFIG.cards_per_row) as i8;
    let xpos = (curr_num - 1) % numbers_per_row + 1;
    let ypos = (curr_num - 1) / numbers_per_row + 1;
    let prev_ypos = (prev_num - 1) / numbers_per_row + 1;

    // if prev and curr are in different rows_per_card and different cards add a vertical space between the 2
    let yshift = ((ypos - 1) / BOARDCONFIG.rows_per_card as i8
        - (prev_ypos - 1) / BOARDCONFIG.rows_per_card as i8)
        * BOARDCONFIG.vcards_space as i8;
    let delta_y = (ypos - prev_ypos) * (1 + BOARDCONFIG.vnumbers_space as i8) + yshift;

    // if prev and curr are in different rows, shift down and reset xpos
    let prev_xpos = if delta_y == 0 {
        (prev_num - 1) % numbers_per_row + 1
    } else {
        0
    };

    // if prev and curr are in the same row but different cards add a horizontal space between the 2
    let xshift = ((xpos - 1) / BOARDCONFIG.cols_per_card as i8
        - (prev_xpos - 1) / BOARDCONFIG.cols_per_card as i8)
        * BOARDCONFIG.hcards_space as i8;

    let delta_x = (xpos - prev_xpos - 1) * (2 + BOARDCONFIG.hnumbers_space as i8)
        + BOARDCONFIG.hnumbers_space as i8
        + xshift;

    DeltaPos {
        delta_x: delta_x as u8,
        delta_y: delta_y as u8,
    }
}

pub fn print_board(board: &[NumberEntry], extracted: u8) {
    let mut sorted_board = board.to_vec();
    sorted_board.sort_by_key(|entry| entry.number);
    let mut prev_num = 0;
    for entry in sorted_board.iter() {
        let curr_num = entry.number;
        for _ in 0..downrightshift(prev_num, curr_num).delta_y {
            println!();
        }
        let spaces = " ".repeat(downrightshift(prev_num, curr_num).delta_x as usize);

        print!("{spaces}");
        if curr_num == extracted {
            print!("\x1b[1;32m{curr_num:2}\x1b[0m"); // Bold green for the last number
        } else if entry.is_marked {
            print!("\x1b[1;33m{curr_num:2}\x1b[0m"); // Bold yellow for marked numbers
        } else {
            print!("{curr_num:2}");
        }
        prev_num = curr_num;
    }
}

// Function to output the last n previous numbers from the board
pub fn print_last_numbers(board: &[NumberEntry], n: usize) -> Vec<u8> {
    if board.len() <= 1 {
        return Vec::new();
    }

    let available_previous = board.len() - 1;
    let numbers_to_show = std::cmp::min(n, available_previous);
    let start_index = board.len() - numbers_to_show - 1;
    let end_index = board.len() - 1;

    let mut result: Vec<u8> = board[start_index..end_index]
        .iter()
        .map(|entry| entry.number)
        .collect();
    result.reverse();
    result
}

pub fn show_on_terminal(
    board: &[NumberEntry],
    pouch: &[u8],
    extracted: u8,
    scorecard: &mut u8,
    itemsleft: usize,
) {
    println!("Last number: \x1b[1;32m{extracted}\x1b[0m");
    println!("Previous numbers: {:?}", print_last_numbers(board, 3));
    println!("\nCurrent board:");
    print_board(board, extracted);
    println!();

    // Mark numbers only if scorecard reaches a NEW goal
    match *scorecard {
        2 => println!("\n\x1b[1;33mTWO in line\x1b[0m"),
        3 => println!("\n\x1b[1;33mTHREE in line\x1b[0m"),
        4 => println!("\n\x1b[1;33mFOUR in line\x1b[0m"),
        5 => println!("\n\x1b[1;33mFIVE in line\x1b[0m"),
        x if x == NUMBERSPERCARD as u8 => println!("\n\x1b[1;33mBINGO!!!\x1b[0m"),
        _ => {}
    }

    match itemsleft {
        0 => println!("\nThe pouch is empty!"),
        _ => {
            println!("\nRemaining in pouch {itemsleft}:");
            for &pouch_num in pouch {
                print!("{pouch_num:2} ");
            }
            println!();
        }
    }

    println!();
}

pub fn hitkey () -> bool {
    println!("\nPress any key to continue or ESC to exit");

    // Enable raw mode to capture individual key presses
    enable_raw_mode().unwrap();

    // Clear any pending events in the buffer
    while event::poll(std::time::Duration::from_millis(0)).unwrap() {
        event::read().unwrap();
    }

    // Wait for a key press
    let result = loop {
        if let Ok(Event::Key(key_event)) = event::read() {
            // Only process key press events, not key release events
            if key_event.kind == event::KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Esc => {
                        break true; // Exit the entire program
                    }
                    _ => {
                        break false; // Continue with the game
                    }
                }
            }
        }
    };

    disable_raw_mode().unwrap();
    print!("\x1Bc"); // Clear the screen

    match result {
        true => {
            println!("Exiting the game.\n");
        },
        false => {
            println!("Continuing the game...\n");
        }
    }

    result
}

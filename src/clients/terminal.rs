// src/terminal.rs
// This module handles terminal input/output for the Tombola game.

use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use crate::defs::{BOARDCONFIG, Number, Colors};
use crate::board::Board;

pub struct DeltaPos {
    pub delta_x: u8,
    pub delta_y: u8,
}

// Function to calculate the horizontal and vertical shifts
#[must_use] pub fn downrightshift(prev_num: Number, curr_num: Number) -> DeltaPos {
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

pub fn print_board(board: &Board) {
    let sorted_entries = board.get_sorted_entries();
    let mut prev_num = 0;
    // Get the last extracted number from the board
    let extracted = board.get_numbers().last().copied().unwrap_or(0);

    for (curr_num, is_marked) in &sorted_entries {
        for _ in 0..downrightshift(prev_num, *curr_num).delta_y {
            println!();
        }
        let spaces = " ".repeat(downrightshift(prev_num, *curr_num).delta_x as usize);

        print!("{spaces}");
        if *curr_num == extracted {
            print!("{}{curr_num:2}{}", Colors::green(), Colors::reset()); // Bold green for the last number
        } else if *is_marked {
            print!("{}{curr_num:2}{}", Colors::yellow(), Colors::reset()); // Bold yellow for marked numbers
        } else {
            print!("{curr_num:2}");
        }
        prev_num = *curr_num;
    }
}

// Function to output the last n previous numbers from the board
#[must_use] pub fn print_last_numbers(board: &Board, n: usize) -> Vec<Number> {
    board.get_last_numbers(n)
}

pub enum KeyAction {
    Extract,  // Enter key pressed
    Exit,     // ESC key pressed
    Refresh,  // F5 key pressed for screen update
}

#[must_use] pub fn wait_for_user_action() -> KeyAction {
    println!("\nPress ENTER to extract a number, F5 to refresh screen, or ESC to exit");

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
                        break KeyAction::Exit; // Exit the entire program
                    }
                    KeyCode::Enter => {
                        break KeyAction::Extract; // Extract a number
                    }
                    KeyCode::F(5) => {
                        break KeyAction::Refresh; // Refresh the screen
                    }
                    _ => {
                        // For any other key, continue waiting
                        continue;
                    }
                }
            }
        }
    };

    disable_raw_mode().unwrap();
    print!("\x1Bc"); // Clear the screen

    result
}

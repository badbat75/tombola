use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};

struct BoardStruct {
    cols_per_card: u8,
    rows_per_card: u8,
    cards_per_row: u8,
    cards_per_col: u8,
    hnumbers_space: u8,
    vnumbers_space: u8,
    hcards_space: u8,
    vcards_space: u8,
}

const BOARDCONFIG: BoardStruct = BoardStruct {
    cols_per_card: 5, // number of columns in a card
    rows_per_card: 3, // number of rows in a card
    cards_per_row: 2, // number of cards in a row
    cards_per_col: 3, // number of cards in a column
    hnumbers_space: 2, // space between numbers in the same row
    vnumbers_space: 1, // space between numbers in the same column
    hcards_space: 2, // space between cards in the same row
    vcards_space: 1, // space between cards in the same column
};

const FIRSTNUMBER: u8 = 1;
const LASTNUMBER: u8 = BOARDCONFIG.cols_per_card * BOARDCONFIG.rows_per_card * BOARDCONFIG.cards_per_row * BOARDCONFIG.cards_per_col;

#[derive(Clone)]
struct NumberEntry {
    number: u8,
    is_marked: bool,
}
struct DeltaPos {
    delta_x: u8,
    delta_y: u8,
}

// Function to calculate the horizontal and vertical shifts
fn downrightshift (prev_num: u8, curr_num: u8) -> DeltaPos {
    let prev_num = prev_num as i8;
    let curr_num = curr_num as i8;
    let numbers_per_row=(BOARDCONFIG.cols_per_card * BOARDCONFIG.cards_per_row) as i8;
    let xpos = (curr_num - 1) % numbers_per_row + 1;
    let ypos = (curr_num - 1) / numbers_per_row + 1;
    let prev_ypos = (prev_num - 1) / numbers_per_row + 1;

    // if prev and curr are in different rows_per_card and different cards add a vertical space between the 2
    let yshift = ((ypos - 1) / BOARDCONFIG.rows_per_card as i8 - (prev_ypos - 1) / BOARDCONFIG.rows_per_card as i8) * BOARDCONFIG.vcards_space as i8;
    let delta_y = (ypos - prev_ypos) * (1 + BOARDCONFIG.vnumbers_space as i8) + yshift;

    // if prev and curr are in different rows, shift down and reset xpos
    let prev_xpos = if delta_y == 0 {
        ( prev_num - 1 ) % numbers_per_row + 1
    } else {
        0
    };

    // if prev and curr are in the same row but different cards add a horizontal space between the 2
    let xshift = ((xpos - 1) / BOARDCONFIG.cols_per_card as i8 - (prev_xpos - 1) / BOARDCONFIG.cols_per_card as i8) * BOARDCONFIG.hcards_space as i8;

    let delta_x = (xpos - prev_xpos - 1) * (2 + BOARDCONFIG.hnumbers_space as i8) + BOARDCONFIG.hnumbers_space as i8 + xshift;

    DeltaPos { delta_x: delta_x as u8, delta_y: delta_y as u8 }
}

fn print_board(board: &[NumberEntry], extracted: u8) {
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
fn print_last_numbers(board: &[NumberEntry], n: usize)  -> Vec<u8> {
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

fn score_check(board: &mut [NumberEntry], extracted: u8, scorecard: &mut u8) {
    let numbers_per_row = (BOARDCONFIG.cols_per_card * BOARDCONFIG.cards_per_row) as i8;
    
    // Store the previous scorecard value BEFORE any updates
    let previous_scorecard = *scorecard;
    
    // Calculate extracted number's position for line checking
    let extracted_ypos = (extracted as i8 - 1) / numbers_per_row + 1;
    let extracted_xpos = (extracted as i8 - 1) % numbers_per_row;
    let extracted_card = extracted_xpos / BOARDCONFIG.cols_per_card as i8;
    
    // Check for complete cards (15 numbers) first
    let total_cards = (BOARDCONFIG.cards_per_row * BOARDCONFIG.cards_per_col) as i8;
    let numbers_per_card = (BOARDCONFIG.cols_per_card * BOARDCONFIG.rows_per_card) as i8;
    
    let mut bingo_found = false;
    for card_index in 0..total_cards {
        let card_row = card_index / BOARDCONFIG.cards_per_row as i8;
        let card_col = card_index % BOARDCONFIG.cards_per_row as i8;
        
        let mut card_numbers_found = 0;
        let mut card_numbers: Vec<u8> = Vec::new();
        
        // Check all numbers in this card
        for row in 0..BOARDCONFIG.rows_per_card as i8 {
            for col in 0..BOARDCONFIG.cols_per_card as i8 {
                let number = 1 + (card_row * BOARDCONFIG.rows_per_card as i8 + row) * numbers_per_row + 
                           card_col * BOARDCONFIG.cols_per_card as i8 + col;
                
                card_numbers.push(number as u8);
                
                if board.iter().any(|entry| entry.number == number as u8) {
                    card_numbers_found += 1;
                }
            }
        }
        
        // If this card is complete (15 numbers), set scorecard to 15
        if card_numbers_found == numbers_per_card {
            *scorecard = numbers_per_card as u8;
            bingo_found = true;
            break;
        }
    }
    
    // Only do line scoring if no BINGO was found
    if !bingo_found {
        // Count numbers in the same row and card as the extracted number (for line scoring)
        let mut same_row_card_count = 0;
        
        for entry in board.iter() {
            let num = entry.number;
            if num == extracted {
                continue; // Skip the extracted number itself for counting
            }
            
            let num_ypos = (num as i8 - 1) / numbers_per_row + 1;
            let num_xpos = (num as i8 - 1) % numbers_per_row;
            let num_card = num_xpos / BOARDCONFIG.cols_per_card as i8;
            
            // Check if same row and same card
            if num_ypos == extracted_ypos && num_card == extracted_card {
                same_row_card_count += 1;
            }
        }
        
        // Update scorecard for line scoring
        // Only update if we haven't achieved this goal before
        let current_line_score = same_row_card_count + 1;
        if current_line_score > *scorecard { 
            *scorecard = current_line_score;
        }
    }

    // Mark numbers only if scorecard reaches a NEW goal
    match *scorecard {
        2 => println!("\nTWO in line"),
        3 => println!("\nTHREE in line"),
        4 => println!("\nFOUR in line"),
        5 => println!("\nFIVE in line"),
        x if x == numbers_per_card as u8 => println!("\nBINGO!!!"),
        _ => {}
    }

    // Mark numbers based on scorecard achievement
    // Only mark if we just achieved a NEW score (higher than previous)
    let is_new_achievement = *scorecard > previous_scorecard;
    
    if is_new_achievement {
        match *scorecard {
            2..=5 => {
                // First, unmark all numbers (reset previous markings)
                for entry in board.iter_mut() {
                    entry.is_marked = false;
                }
                
                // Only mark numbers that are actually part of the current scoring line
                for entry in board.iter_mut() {
                    let num = entry.number;
                    
                    let num_ypos = (num as i8 - 1) / numbers_per_row + 1;
                    let num_xpos = (num as i8 - 1) % numbers_per_row;
                    let num_card = num_xpos / BOARDCONFIG.cols_per_card as i8;
                    
                    // Only mark if it's in the same row and card as the extracted number
                    if num_ypos == extracted_ypos && num_card == extracted_card {
                        entry.is_marked = true;
                    }
                }
            },
            x if x == numbers_per_card as u8 => {
                // First, unmark all numbers (BINGO overrides everything)
                for entry in board.iter_mut() {
                    entry.is_marked = false;
                }
                
                // Mark all numbers in the completed card
                for card_index in 0..total_cards {
                    let card_row = card_index / BOARDCONFIG.cards_per_row as i8;
                    let card_col = card_index % BOARDCONFIG.cards_per_row as i8;
                    
                    let mut card_numbers_found = 0;
                    let mut card_numbers: Vec<u8> = Vec::new();
                    
                    // Collect all numbers in this card (regardless of extraction)
                    for row in 0..BOARDCONFIG.rows_per_card as i8 {
                        for col in 0..BOARDCONFIG.cols_per_card as i8 {
                            let number = 1 + (card_row * BOARDCONFIG.rows_per_card as i8 + row) * numbers_per_row + 
                                       card_col * BOARDCONFIG.cols_per_card as i8 + col;
                            
                            card_numbers.push(number as u8);
                            
                            if board.iter().any(|entry| entry.number == number as u8) {
                                card_numbers_found += 1;
                            }
                        }
                    }
                    
                    // Debug: Show what we found for this card
                    
                    // If this card is complete, mark all its extracted numbers
                    if card_numbers_found == numbers_per_card {
                        for entry in board.iter_mut() {
                            if card_numbers.contains(&entry.number) {
                                entry.is_marked = true;
                            }
                        }
                        break;
                    }
                }
            },
            _ => {}
        }
    }
}

// Function to wait for a key press and return true if ESC is pressed, false otherwise
fn hitkey () -> bool {
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
    result
}

fn main() {
    let mut pouch: Vec<u8> = (FIRSTNUMBER..=LASTNUMBER).collect();
    let mut board: Vec<NumberEntry> = Vec::new();
    let mut itemsleft = pouch.len();
    let mut scorecard = 0;

    while ! pouch.is_empty() {
        match hitkey() {
            true => {
                println!("Exiting the game.\n");
                break;
            },
            false => {
                println!("Continuing the game...\n");
            }
        };

        let random_index = rand::random_range(0..itemsleft);
        let extracted = pouch.remove(random_index);
        board.push(NumberEntry {
            number: extracted,
            is_marked: false,
        });
        itemsleft = pouch.len();

        println!("Last number: \x1b[1;32m{extracted}\x1b[0m");
        println!("Previous numbers: {:?}", print_last_numbers(&board, 3));
        score_check(&mut board, extracted, &mut scorecard);
        println!("\nCurrent board:");
        print_board(&board, extracted);
        println!();


        match itemsleft {
            0 => println!("\nThe pouch is empty!"),
            _ => {
                println!("\nRemaining in pouch {itemsleft}:");
                for &pouch_num in &pouch {
                    print!("{pouch_num:2} ");
                }
                println!();
            },
        }
        println!();
        if scorecard == 15 { break };
    }
}
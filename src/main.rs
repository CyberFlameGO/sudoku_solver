// #![feature(const_fn)]

use std::fs::File;
use std::io::Read;
use crossbeam_channel::{unbounded, Sender};

use itertools::Itertools;
use std::time::Instant;

const ROWS: [[u8; 9]; 9] = [[00, 01, 02, 03, 04, 05, 06, 07, 08],
                            [09, 10, 11, 12, 13, 14, 15, 16, 17],
                            [18, 19, 20, 21, 22, 23, 24, 25, 26],
                            [27, 28, 29, 30, 31, 32, 33, 34, 35],
                            [36, 37, 38, 39, 40, 41, 42, 43, 44],
                            [45, 46, 47, 48, 49, 50, 51, 52, 53],
                            [54, 55, 56, 57, 58, 59, 60, 61, 62],
                            [63, 64, 65, 66, 67, 68, 69, 70, 71],
                            [72, 73, 74, 75, 76, 77, 78, 79, 80]];

const COLS: [[u8; 9]; 9] = [[00, 09, 18, 27, 36, 45, 54, 63, 72],
                            [01, 10, 19, 28, 37, 46, 55, 64, 73],
                            [02, 11, 20, 29, 38, 47, 56, 65, 74],
                            [03, 12, 21, 30, 39, 48, 57, 66, 75],
                            [04, 13, 22, 31, 40, 49, 58, 67, 76],
                            [05, 14, 23, 32, 41, 50, 59, 68, 77],
                            [06, 15, 24, 33, 42, 51, 60, 69, 78],
                            [07, 16, 25, 34, 43, 52, 61, 70, 79],
                            [08, 17, 26, 35, 44, 53, 62, 71, 80]];

const SQRS: [[u8; 9]; 9] = [[00, 01, 02, 09, 10, 11, 18, 19, 20],
                            [03, 04, 05, 12, 13, 14, 21, 22, 23],
                            [06, 07, 08, 15, 16, 17, 24, 25, 26],
                            [27, 28, 29, 36, 37, 38, 45, 46, 47],
                            [30, 31, 32, 39, 40, 41, 48, 49, 50],
                            [33, 34, 35, 42, 43, 44, 51, 52, 53],
                            [54, 55, 56, 63, 64, 65, 72, 73, 74],
                            [57, 58, 59, 66, 67, 68, 75, 76, 77],
                            [60, 61, 62, 69, 70, 71, 78, 79, 80]];

const fn get_row(index: u8) -> u8 {
    index / 9
}

const fn get_col(index: u8) -> u8 {
    index % 9
}

const fn get_sqr(index: u8) -> u8 {
    (get_col(index) / 3) + (get_row(index) / 3) * 3
}

//const fn get_sqr_rel(index: usize) -> usize {
//    ((get_col(index) % 3) + (get_row(index) * 3)) % 9
//}

fn get_influences(index: u8) -> Vec<u8> {
    let mut influences = Vec::new();

    ROWS[get_row(index) as usize].iter().for_each(|x| influences.push(x.clone()));
    COLS[get_col(index) as usize].iter().for_each(|x| influences.push(x.clone()));
    SQRS[get_sqr(index) as usize].iter().for_each(|x| influences.push(x.clone()));

    influences.iter().filter(|x| **x != index).unique().map(|x| x.clone()).collect()
}

fn main() {
    let mut file = File::open("test_a.txt").expect("No puzzle file");

    let mut buf = String::new();

    file.read_to_string(&mut buf).expect("Failed to read file to string");

    let (completed_tx, completed_rx) = unbounded();

    let threads = usize::from_str(std::env::var("RUST_SUDOKU_THREADS")
        .unwrap_or(String::from("8")).as_str()).unwrap();

    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();

    let board = Board::try_from_str(&buf).expect("bad puzzle");

    let start = Instant::now();
    pool.scope(|scope| process_board(board, completed_tx, scope));
    let duration = start.elapsed();

    println!("Process complete. Operation took {} seconds", duration.as_secs());

    // debug_process_board(board, completed_tx);

    let mut solved_boards = Vec::new();
    let mut failed_boards = Vec::new();

    while let Ok(res) = completed_rx.recv() {
        match res {
            BoardResult::Solved(b) => solved_boards.push(b),
            BoardResult::Failed(b) => failed_boards.push(b),
            BoardResult::Branch(_) => panic!("Branch result returned to completion, this should not happen!"),
        }
    }

    println!("Solved Boards: {}", solved_boards.len());
    println!("Failed Boards: {}", failed_boards.len());
}

fn process_board(board: Board, completed_tx: Sender<BoardResult>, scope: &rayon::Scope<'_>) {
    use BoardResult::*;
    match board.try_solve() {
        Solved(b) => {
            completed_tx.send(Solved(b)).expect("completed_rx disposed?");
        }
        Failed(b) => {
            completed_tx.send(Failed(b)).expect("completed_rx disposed?");
        }
        Branch(boards) => boards
            .into_iter()
            .for_each(|b| {
                let completed_tx = completed_tx.clone();
                scope.spawn(|scope| process_board(b, completed_tx, scope))
            }),
    };
}

fn debug_process_board(board: Board, completed_tx: Sender<BoardResult>) {
//    println!("Starting new board. Hit enter to continue.");
//    { std::io::stdin().read_line(&mut String::new()); }

    use BoardResult::*;
    match board.try_solve() {
        Solved(b) => {
            completed_tx.send(Solved(b)).expect("completed_rx disposed?");
        }
        Failed(b) => {
            completed_tx.send(Failed(b)).expect("completed_rx disposed?");
        }
        Branch(boards) => boards
            .into_iter()
            .for_each(|b| {
                let completed_tx = completed_tx.clone();
                debug_process_board(b, completed_tx)
            })
    }
}

#[derive(Clone)]
pub struct Board {
    tiles: [Tile; 81],
}

#[derive(Clone, Copy)]
pub struct Tile {
    pub index: u8,
    pub value: Option<u8>,
    pub hints: [bool; 9],
}

impl Board {
    pub fn try_from_str(src: &str) -> Result<Board, String> {
        let mut tiles= [Tile::default(); 81];


        let collected = src.lines()
            .filter(|s| !s.is_empty())
            .flat_map(|content| {
                let mut index = 0u8;
                content.chars()
                    .filter(|c| {
                        match c {
                            '0'..='9' => true,
                            _ => false,
                        }
                    })
                    .map(move |c| {
                        let value = char_to_maxnine(c);
                        let hints = match &value {
                            Some(v) => {
                                let mut h = [false; 9];
                                h[*v as usize] = true;
                                h
                            },
                            None => [true; 9],
                        };

                        index += 1;

                        Tile {
                            index: (index - 1),
                            value,
                            hints,
                        }
                    })
            });

        if tiles.len() != 81 {
            return Err(String::from("More than 81 tiles collected"))
        }

        collected.for_each(|t| tiles[t.index as usize] = t);

        Ok(Board { tiles })
    }

    pub fn try_solve(mut self) -> BoardResult {
        let mut progress;

        let mut iteration = 1u32;
        loop {
            progress = false;
//            println!("Iteration {}", iteration);
            iteration += 1;

            for tile_index in 0..self.tiles.len() {
//                println!("Tile {} hints: {:?}", tile_index, self.tiles[tile_index].hints);
//                println!("Tile {} influences: {:?}", tile_index, get_influences(tile_index));
                if let Some(v) = self.tiles[tile_index].value {
//                    println!("Skipping tile, has value {}", v);
                    continue;
                }

                for inf in get_influences(tile_index as u8) {
                    match self.tiles[inf as usize].value {
                        Some(v) => {
                            let hint = &mut self.tiles[tile_index].hints[v as usize];
                            if *hint {
                                *hint = false;

//                                println!("PROGRESS!! Tile {}'s hint value {} cleared", tile_index, v);
                                progress = true;
                            }
                        }
                        None => {},
                    }
                }

                let hints = self.tiles[tile_index].hints.iter()
                    .filter(|h| **h).count();

                match hints {
                    1 => {
                        let val = self.tiles[tile_index].hints.iter()
                            .position(|h| *h).map(|v| v as u8);

                        self.tiles[tile_index].value = val.clone();

//                        println!("PROGRESS!! Tile {} assigned {}", tile_index, val.unwrap());

                        progress = true;
                    }
                    0 =>  {
//                        println!("Tile with 0 hints remaining encountered. This board is failed.");
                        return BoardResult::Failed(self)
                    },
                    _ => {},
                }
            }

            if progress == false {
//                println!("Iteration {} had no progress, evaluating state", iteration);
                break;
            }
        }

        match self.tiles.iter().all(|t| t.value.is_some()) {
            true => {
                println!("BOARD SOLVED!!");
                BoardResult::Solved(self)
            },
            false => {
//                println!("Board not solved. Branching.");
                let branchtile = self.tiles.iter()
                    .find(|t| t.value.is_none()).unwrap();

                let mut branches = Vec::new();

                for (i, h) in branchtile.hints.iter().enumerate() {
                    if !h { continue; }

//                    println!("Branching on Tile {} for Value {}", branchtile.index, i);

                    let mut branch = self.clone();

                    let branchtile = &mut branch.tiles[branchtile.index as usize];

                    branchtile.value = Some(i as u8);
                    branchtile.hints = Default::default();
                    branchtile.hints[i] = true;

                    branches.push(branch);
                }

                BoardResult::Branch(branches)
            }
        }
    }
}

pub enum BoardResult {
    Solved(Board),
    Branch(Vec<Board>),
    Failed(Board)
}

fn char_to_maxnine(c: char) -> Option<u8> {
    match c {
        '1' => Some(0),
        '2' => Some(1),
        '3' => Some(2),
        '4' => Some(3),
        '5' => Some(4),
        '6' => Some(5),
        '7' => Some(6),
        '8' => Some(7),
        '9' => Some(8),
        _   => None,
    }
}

impl Default for Tile {
    fn default() -> Self {
        Tile {
            index: 0,
            value: None,
            hints: [true; 9],
        }
    }
}

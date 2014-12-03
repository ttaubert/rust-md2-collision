/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this file,
* You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate "rust-md2" as md2;

use md2::{S, S2, md2_compress};
use std::collections::HashMap;
use std::collections::hash_map::{Occupied, Vacant};
use std::slice::bytes::copy_memory;

type Collision = Vec<Vec<u8>>;
type Collisions = Vec<Collision>;

fn find_collisions(rows: uint) -> Collisions {
    let mut values = create_initial_state(rows);
    let mut bytes = Vec::from_elem(16 - rows, 0u8);
    let mut collisions: HashMap<Vec<u8>,Collision> = HashMap::new();

    loop {
        copy_memory(values[rows].slice_mut(17, 17 + 16 - rows), bytes.as_slice());
        copy_memory(values[rows].slice_mut(17 + 16, 17 + 16 - rows + 16), bytes.as_slice());

        for row in range(rows + 1, 18) {
            // Fill row.
            for i in range(1, 49) {
                values[row][i] = S[values[row][i - 1] as uint] ^ values[row - 1][i];
            }

            // Next t value.
            values[row + 1][0] = values[row][48] + (row as u8) - 1;
        }

        let key = Vec::from_fn(17 - rows, |row| values[rows + 2 + row][0]);

        match collisions.entry(key) {
            Vacant(entry) => { entry.set(vec!(bytes.clone())); },
            Occupied(mut entry) => { entry.get_mut().push(bytes.clone()); }
        };

        if !increase(bytes.as_mut_slice()) {
            break;
        }
    }

    // Compute original messages for each collision.
    collisions.values().filter(|x| x.len() > 1).map(|collision| {
        collision.iter().map(|bytes| {
            copy_memory(values[rows].slice_mut(17, 17 + 16 - rows), bytes.as_slice());
            copy_memory(values[rows].slice_mut(17 + 16, 17 + 16 - rows + 16), bytes.as_slice());

            // Fill upper rectangles.
            for row in range(1, rows + 1).rev() {
                for col in range(17, 32 - row + 2) {
                    values[row - 1][col] = S[values[row][col - 1] as uint] ^ values[row][col];
                }
            }

            values[0].slice(17, 33).to_vec()
        }).collect()
    }).collect()
}

fn create_initial_state(rows: uint) -> [[u8, ..49], ..19] {
    let mut values = [[0u8, ..49], ..19];

    for row in range(1, rows + 1) {
        // Fill row of T1.
        for i in range(1, 17) {
            values[row][i] = S[values[row][i - 1] as uint] ^ values[row - 1][i];
        }

        // Last bytes are equal.
        values[row][32] = values[row][16];
        values[row][48] = values[row][16];

        // Next t value.
        values[row + 1][0] = values[row][48] + (row as u8) - 1;
    }

    // Compute triangles.
    for col in range(0, rows) {
        for row in range(2 + col, rows + 1).rev() {
            values[row][32 - col - 1] = S2[(values[row][32 - col] ^ values[row - 1][32 - col]) as uint];
            values[row][48 - col - 1] = S2[(values[row][48 - col] ^ values[row - 1][48 - col]) as uint];
        }
    }

    values
}

fn increase(num: &mut [u8]) -> bool {
    let len = num.len();

    for i in range(0, len).rev() {
        if num[i] == 255 {
            continue;
        }

        num[i] += 1;

        for i in range(i + 1, len) {
            num[i] = 0;
        }

        return true;
    }

    false
}

fn check_collisions(collisions: &Collisions) -> bool {
    let empty = [0u8, ..16];

    for collision in collisions.iter() {
        let mut first_hash: Option<[u8, ..16]> = None;

        for msg in collision.iter() {
            let md2 = md2_compress(&empty, msg.as_slice());

            match first_hash {
                Some(hash) => {
                    if hash != md2 {
                        return false;
                    }
                }
                None => first_hash = Some(md2)
            };
        }
    }

    true
}

fn main() {
    let collisions = find_collisions(14);
    if !check_collisions(&collisions) {
        panic!("invalid collision found :(");
    }

    println!("Found {} collisions.", collisions.iter().count());
}

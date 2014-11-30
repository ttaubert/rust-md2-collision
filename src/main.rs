/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this file,
* You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate "rust-md2" as md2;

use md2::{S, S2, md2_compress};
use std::collections::HashMap;

pub fn main() {
    let rows = 14;
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
      for row in range(0, rows - col - 1) {
        let row2 = rows - row;
        values[row2][32 - col - 1] = S2[(values[row2][32 - col] ^ values[row2 - 1][32 - col]) as uint];
        values[row2][48 - col - 1] = S2[(values[row2][48 - col] ^ values[row2 - 1][48 - col]) as uint];
      }
    }

    let mut collisions: HashMap<(u8,u8,u8),Vec<u16>> = HashMap::new();

    // TODO random bytes
    for bytes in range(0u32, 256 * 256) {
        // TODO set bytes
        values[14][17] = (bytes >> 8) as u8;
        values[14][18] = bytes as u8;
        values[14][33] = (bytes >> 8) as u8;
        values[14][34] = bytes as u8;

        for row in range(rows + 1, rows + 4) {
            // Fill row.
            for i in range(1, 49) {
                values[row][i] = S[values[row][i - 1] as uint] ^ values[row - 1][i];
            }

            // Next t value.
            values[row + 1][0] = values[row][48] + (row as u8) - 1;
        }

        let key = (values[16][0], values[17][0], values[18][0]);

        if !collisions.contains_key(&key) {
            collisions.insert(key, vec!());
        }

        match collisions.get_mut(&key) {
            Some(vec) => vec.push(bytes as u16),
            None => panic!("unreachable")
        }
    }

    // Compute original messages for each collision.
    let mut count = 0u;
    for collision in collisions.values().filter(|x| x.len() > 1) {
        count += 1;
        for bytes in collision.iter() {
            // TODO set bytes
            values[14][17] = (*bytes >> 8) as u8;
            values[14][18] = *bytes as u8;
            values[14][33] = (*bytes >> 8) as u8;
            values[14][34] = *bytes as u8;

            // Fill upper rectangles.
            for row in range(0, 14) {
                let row2 = rows - row;
                for col in range(17, 32 - row2 + 2) {
                    values[row2 - 1][col] = S[values[row2][col - 1] as uint] ^ values[row2][col];
                }
            }

            let empty = [0u8, ..16];
            let msg = values[0].slice(17, 33).iter().fold(String::new(), |a, &b| format!("{}{:02x}", a, b));
            let hash = md2_compress(&empty, values[0].slice(17, 33)).iter().fold(String::new(), |a, &b| format!("{}{:02x}", a, b));
            println!("msg = {}, hash = {}", msg, hash);
        }

        println!("");
    }

    println!("Found {} collisions.", count);
}

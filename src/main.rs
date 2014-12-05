/* This Source Code Form is subject to the terms of the Mozilla Public
* License, v. 2.0. If a copy of the MPL was not distributed with this file,
* You can obtain one at http://mozilla.org/MPL/2.0/. */

extern crate "rust-md2" as md2;

use md2::{SBOX, SBOXI, md2_compress};
use std::collections::HashMap;
use std::collections::hash_map::{Occupied, Vacant};
use std::slice::bytes::{copy_memory, MutableByteVector};

type Collision = Vec<Vec<u8>>;
type Collisions = Vec<Collision>;

struct ByteRange {
  v: Vec<u8>
}

impl ByteRange {
  fn new(n: uint) -> ByteRange {
    ByteRange { v: Vec::from_elem(n, 0u8) }
  }
}

impl Iterator<Vec<u8>> for ByteRange {
  fn next(&mut self) -> Option<Vec<u8>> {
    for i in range(0, self.v.len()).rev() {
      if self.v[i] == 255 {
        continue;
      }

      // Increase.
      self.v[i] += 1;

      // Zero all bytes right of the current index.
      self.v.slice_from_mut(i + 1).set_memory(0);

      return Some(self.v.clone());
    }

    None
  }
}

fn find_collisions(state: [[u8, ..49], ..19], k: uint) -> Collisions {
  let rows = 16 - k;
  let mut state = state;
  let mut collisions: HashMap<Vec<u8>,Collision> = HashMap::new();

  for bytes in ByteRange::new(k) {
    copy_memory(state[rows].slice_mut(17, 17 + k), bytes.as_slice());
    copy_memory(state[rows].slice_mut(17 + 16, 17 + k + 16), bytes.as_slice());

    for row in range(rows + 1, 18) {
      // Fill row.
      for i in range(1, 49) {
        state[row][i] = SBOX[state[row][i - 1] as uint] ^ state[row - 1][i];
      }

      // Next t value.
      state[row + 1][0] = state[row][48] + (row as u8) - 1;
    }

    let key = Vec::from_fn(17 - rows, |row| state[rows + 2 + row][0]);

    match collisions.entry(key) {
      Vacant(entry) => { entry.set(vec!(bytes)); },
      Occupied(mut entry) => { entry.get_mut().push(bytes); }
    };
  }

  // Compute original messages for each collision.
  collisions.values().filter(|x| x.len() > 1).map(|collision| {
    collision.iter().map(|bytes| {
      copy_memory(state[rows].slice_mut(17, 17 + k), bytes.as_slice());
      copy_memory(state[rows].slice_mut(17 + 16, 17 + k + 16), bytes.as_slice());

      // Fill upper rectangles.
      for row in range(1, rows + 1).rev() {
        for col in range(17, 32 - row + 2) {
          state[row - 1][col] = SBOX[state[row][col - 1] as uint] ^ state[row][col];
        }
      }

      state[0].slice(17, 33).to_vec()
    }).collect()
  }).collect()
}

fn create_initial_state(k: uint) -> [[u8, ..49], ..19] {
  let rows = 16 - k;
  let mut state = [[0u8, ..49], ..19];

  for row in range(1, rows + 1) {
    // Fill row of T1.
    for i in range(1, 17) {
      state[row][i] = SBOX[state[row][i - 1] as uint] ^ state[row - 1][i];
    }

    // Last bytes are equal.
    state[row][32] = state[row][16];
    state[row][48] = state[row][16];

    // Next t value.
    state[row + 1][0] = state[row][48] + (row as u8) - 1;
  }

  // Compute triangles.
  for col in range(0, rows) {
    for row in range(2 + col, rows + 1).rev() {
      state[row][32 - col - 1] = SBOXI[(state[row][32 - col] ^ state[row - 1][32 - col]) as uint];
      state[row][48 - col - 1] = SBOXI[(state[row][48 - col] ^ state[row - 1][48 - col]) as uint];
    }
  }

  state
}

fn check_collision(collision: &Collision) -> bool {
  let empty = [0u8, ..16];
  let mut first_hash: Option<[u8, ..16]> = None;
  let mut hashes = collision.iter().map(|msg| {
    md2_compress(&empty, msg.as_slice())
  });

  hashes.all(|md2| {
    first_hash.map_or_else(|| { first_hash = Some(md2); true }, |v| v == md2)
  })
}

fn main() {
  let k = 2;
  let state = create_initial_state(k);
  let collisions = find_collisions(state, k);

  // Check that the colliding messages we found generate the same hashes.
  if !collisions.iter().all(check_collision) {
    panic!("invalid collision found :(");
  }

  println!("Found {} collisions.", collisions.iter().count());
}

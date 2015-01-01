/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this file,
 * You can obtain one at http://mozilla.org/MPL/2.0/. */

#![feature(slicing_syntax)]

extern crate "rust-md2" as md2;

use md2::{SBOX, SBOXI};
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::iter::range_inclusive;
use std::slice::bytes::{copy_memory, MutableByteVector};
use std::sync::TaskPool;

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
      self.v[mut i+1..].set_memory(0);

      return Some(self.v.clone());
    }

    None
  }
}

fn create_initial_state(k: uint) -> [[u8, ..49], ..19] {
  let rows = 16 - k;
  let mut state = [[0u8, ..49], ..19];

  for row in range_inclusive(1, rows) {
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
    for row in range_inclusive(2 + col, rows).rev() {
      state[row][32 - col - 1] = SBOXI[(state[row][32 - col] ^ state[row - 1][32 - col]) as uint];
      state[row][48 - col - 1] = SBOXI[(state[row][48 - col] ^ state[row - 1][48 - col]) as uint];
    }
  }

  state
}

pub fn find(k: uint) {
  let pool = TaskPool::new(8u);
  let (tx, rx) = channel();
  let state = create_initial_state(2);

  for byte in range(0u, 1u) {
    let txc = tx.clone();
    let sc = state.clone();

    pool.execute(move || {
      /*// TODO modify T2
      sc[14][19] = byte as u8;
      for i in range_inclusive(20, 32) {
        sc[14][i] = SBOX[sc[14][i - 1] as uint] ^ sc[13][i];
      }

      // TODO modify T3
      sc[14][35] = byte as u8;
      for i in range_inclusive(36, 48) {
        sc[14][i] = SBOX[sc[14][i - 1] as uint] ^ sc[13][i];
      }

      // TODO modify next T1,0 value
      sc[15][0] = sc[14][48] + 13;*/

      txc.send(find_collisions(sc));
    });
  }

  let mut merged: HashMap<Vec<u8>,Collision> = HashMap::new();

  let mut count = 0u;
  for collisions in rx.iter().take(1u) {
    for (key, bytes) in collisions.into_iter() {
      match merged.entry(key) {
        Vacant(entry) => { entry.set(bytes); },
        Occupied(mut entry) => { entry.get_mut().push_all(bytes[]); }
      }
    }

    println!("merged #{}", count);
    count += 1;
  }

  let num = merged.values().filter(|x| x.len() > 1).count();
  println!("count = {}", num);
}

fn find_collisions(state: [[u8, ..49], ..19]) -> HashMap<Vec<u8>,Collision> {
  let k = 2;
  let rows = 16 - k;
  let mut state = state;
  let mut collisions: HashMap<Vec<u8>,Collision> = HashMap::new();

  for bytes in ByteRange::new(k) {
    copy_memory(state[rows][mut 17..17+k], bytes[]);
    copy_memory(state[rows][mut 17+16..17+k+16], bytes[]);

    let state = state[rows][1..];
    assert_eq!(state.len(), 48);
    let msg = decompress(state, 14);

    match collisions.entry(compress(state, 14)) {
      Vacant(entry) => { entry.set(vec!(msg)); },
      Occupied(mut entry) => { entry.get_mut().push(msg); }
    }
  }

  collisions
}

fn compress(state: &[u8], iteration: uint) -> Vec<u8> {
  let mut t = state[47] + iteration as u8 - 1;
  let mut x = state.to_vec();

  for row in range(iteration, 18) {
    for byte in x.iter_mut() {
      *byte ^= SBOX[t as uint];
      t = *byte;
    }
    t += row as u8;
  }

  x[..16].to_vec()
}

fn decompress(state: &[u8], iteration: uint) -> Vec<u8> {
  let mut x = state.to_vec();

  for row in range(0, iteration).rev() {
    for col in range(1, 48).rev() {
      x[col] ^= SBOX[x[col - 1] as uint];
    }

    let t = x[47] + (row as u8) - 1;
    x[0] ^= SBOX[t as uint];
  }

  x[16..32].to_vec()
}

#[cfg(test)]
mod test {
  use find;
  use md2::compress;

  fn check_collision(collision: &Vec<Vec<u8>>) -> bool {
    let empty = [0u8, ..16];
    let mut first_hash: Option<[u8, ..16]> = None;
    let hashes = collision.iter().map(|msg| compress(&empty, msg[]));

    hashes.all(|md2| {
      first_hash.map_or_else(|| { first_hash = Some(md2); true }, |v| v == md2)
    })
  }

  fn check_collisions(k: uint, num: uint, num_total: uint) {
    find(k);
    /*assert!(collisions.iter().all(check_collision));
    assert_eq!(collisions.iter().count(), num);
    assert_eq!(collisions.iter().fold(0u, |acc, coll| acc + coll.len() - 1), num_total);*/
  }

  #[test]
  fn test() {
    //check_collisions(2, 141, 141);
    check_collisions(3, 32744, 32784);
  }
}

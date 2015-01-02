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

pub struct Collisions {
  map: HashMap<Vec<u8>,Vec<Vec<u8>>>
}

impl Collisions {
  pub fn new() -> Collisions {
    Collisions { map: HashMap::new() }
  }

  pub fn add(&mut self, key: Vec<u8>, msg: Vec<u8>) {
    match self.map.entry(key) {
      Vacant(entry) => { entry.set(vec!(msg)); },
      Occupied(mut entry) => { entry.get_mut().push(msg); }
    }
  }

  pub fn merge(&mut self, other: Collisions) {
    for (key, msgs) in other.map.into_iter() {
      match self.map.entry(key) {
        Vacant(entry) => { entry.set(msgs); },
        Occupied(mut entry) => { entry.get_mut().push_all(msgs[]); }
      }
    }
  }

  pub fn validate(&self) -> bool {
    self.map.values().all(|msgs| {
      let empty = [0u8, ..16];
      let mut first_hash: Option<[u8, ..16]> = None;
      let hashes = msgs.iter().map(|msg| md2::compress(&empty, msg[]));

      hashes.all(|md2| {
        first_hash.map_or_else(|| { first_hash = Some(md2); true }, |v| v == md2)
      })
    })
  }

  pub fn count(&self) -> uint {
    self.map.values().fold(0u, |count, msgs| count + msgs.len() - 1)
  }
}

pub fn create_initial_state(k: uint) -> [[u8, ..49], ..19] {
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

pub fn find_collisions(state: &[u8], k: uint) -> Collisions {
  let rows = 16 - k;
  assert_eq!(state.len(), 48);
  let mut state = state.to_vec();
  let mut collisions = Collisions::new();

  let k = 2;
  for bytes in ByteRange::new(k) {
    copy_memory(state[mut 16..16+k], bytes[]);
    copy_memory(state[mut 16+16..16+k+16], bytes[]);

    let key = compress(state[], rows);
    let msg = decompress(state[], rows);
    collisions.add(key, msg);
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
  use Collisions;
  use find_collisions;
  use create_initial_state;
  use std::sync::TaskPool;

  #[test]
  fn test_k2() {
    let state = create_initial_state(2);
    let collisions = find_collisions(state[14][1..], 2);

    assert!(collisions.validate());
    assert_eq!(collisions.count(), 141);
  }

  #[test]
  fn test_k3() {
    let pool = TaskPool::new(8u);
    let (tx, rx) = channel();
    let state = create_initial_state(3);

    for byte in range(0u, 256u) {
      let txc = tx.clone();
      let mut sc = state.clone();

      pool.execute(move || {
        sc[13][19] = byte as u8;
        sc[13][35] = byte as u8;

        txc.send(find_collisions(sc[13][1..], 3));
      });
    }

    // Merge partial results.
    let mut collisions = Collisions::new();
    for partial in rx.iter().take(256u) {
      collisions.merge(partial);
    }

    assert!(collisions.validate());
    assert_eq!(collisions.count(), 32784);
  }
}

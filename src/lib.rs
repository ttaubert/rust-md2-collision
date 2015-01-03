/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this file,
 * You can obtain one at http://mozilla.org/MPL/2.0/. */

#![feature(slicing_syntax)]

extern crate "rust-md2" as md2;

use md2::{SBOX, SBOXI};
use std::iter::{range_inclusive, repeat};
use std::slice::bytes::{copy_memory, MutableByteVector};

struct ByteRange {
  current: Vec<u8>,
  done: bool
}

impl ByteRange {
  fn new(num_bytes: uint) -> ByteRange {
    ByteRange { current: repeat(0u8).take(num_bytes).collect(), done: false }
  }
}

impl Iterator<Vec<u8>> for ByteRange {
  fn next(&mut self) -> Option<Vec<u8>> {
    for i in range(0, self.current.len()).rev() {
      if self.current[i] == 255 {
        continue;
      }

      let bytes = self.current.clone();

      // Increase.
      self.current[i] += 1;

      // Zero all bytes right of the current index.
      self.current.slice_from_mut(i+1).set_memory(0);

      return Some(bytes);
    }

    // Need to special case where all bytes = 255.
    if self.done { None } else { self.done = true; Some(self.current.clone()) }
  }
}

pub struct Candidates {
  range: ByteRange,
  state: Vec<u8>,
  row: uint
}

impl Iterator<(Vec<u8>, Vec<u8>)> for Candidates {
  fn next(&mut self) -> Option<(Vec<u8>, Vec<u8>)> {
    let next = self.range.next();

    // Bail out if we tried all possible combinations.
    if next.is_none() {
      return None;
    }

    // Set bytes for current candidate.
    let bytes = next.unwrap();
    copy_memory(self.state.slice_mut(16, 18), bytes[]);
    copy_memory(self.state.slice_mut(32, 34), bytes[]);

    // Compute the final compression value.
    let cmp = compress(self.state[], self.row);

    // Compute the original message leading to this state.
    let msg = decompress(self.state[], self.row);

    Some((cmp, msg))
  }
}

pub fn candidates(state: &[u8], row: uint) -> Candidates {
  // Test 2^16 combinations.
  Candidates { range: ByteRange::new(2), state: state.to_vec(), row: row }
}

pub fn prefill_row(num_rows: uint) -> Vec<u8> {
  let mut state = [[0u8; 49]; 19];

  for row in range_inclusive(1, num_rows) {
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

  // Compute triangles in T2 and T3.
  for col in range(0, num_rows) {
    for row in range_inclusive(2 + col, num_rows).rev() {
      let xor = state[row][32 - col] ^ state[row - 1][32 - col];

      // We need the inverse S-box to compute triangles.
      state[row][32 - col - 1] = SBOXI[xor as uint];
      state[row][48 - col - 1] = SBOXI[xor as uint];
    }
  }

  // Return the desired row and throw away the first byte (t-values).
  state[num_rows][1..].to_vec()
}

fn compress(state: &[u8], iteration: uint) -> Vec<u8> {
  let mut t = state[47] + iteration as u8 - 1;
  let mut x = state.to_vec();

  // Compute the MD2 compression function from the current state until we
  // have the final compression state that would be fed into the next round.
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

  // Compute the MD2 compression function from the current state backwards
  // until we arrive at the original message that needs to be passed into it
  // to result in the current state.
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
  use candidates;
  use prefill_row;

  use md2::compress;
  use std::collections::HashMap;
  use std::collections::hash_map::Entry::{Occupied, Vacant};
  use std::sync::TaskPool;
  use std::sync::mpsc::channel;

  // Insert the given candidate pair, consisting of the compressed and the
  // original message, into the given hash map.
  fn insert(map: &mut HashMap<Vec<u8>,Vec<Vec<u8>>>, cmp: Vec<u8>, msg: Vec<u8>) {
    match map.entry(cmp) {
      Vacant(entry) => { entry.set(vec!(msg)); }
      Occupied(mut entry) => { entry.get_mut().push(msg); }
    }
  }

  // Validate all colliding entries in the given map to ensure that those
  // messages do indeed collide when computing their compressed values.
  fn validate(map: &HashMap<Vec<u8>,Vec<Vec<u8>>>) -> bool {
    let empty = [0u8; 16];

    // Ignore compressed values with only a single message (no collisions).
    let collisions = map.iter().filter(|&(_, msgs)| msgs.len() > 1);

    collisions.all(|(cmp, msgs)| {
      msgs.iter().all(|msg| compress(&empty, msg[]) == *cmp)
    })
  }

  // Count the number of map entries that have more than a single message.
  // Those will compress to the same final value and thus represent collisions.
  fn count(map: &HashMap<Vec<u8>,Vec<Vec<u8>>>) -> uint {
    map.values().fold(0u, |count, msgs| count + msgs.len() - 1)
  }

  #[test]
  fn test_k2() {
    let state = prefill_row(14);

    // There will be ~2^16 entries (minus collisions).
    let mut map = HashMap::with_capacity(256u * 256u);

    // Iterate and record all candidate pairs.
    for (cmp, msg) in candidates(state[], 14) {
      insert(&mut map, cmp, msg);
    }

    assert!(validate(&map));
    assert_eq!(count(&map), 141);
  }

  #[test]
  fn test_k3() {
    let pool = TaskPool::new(8u);
    let (tx, rx) = channel();
    let state = prefill_row(13);

    for byte in range(0u, 256u) {
      let txc = tx.clone();
      let mut state = state.clone();

      pool.execute(move || {
        // Set the third bytes of T2 and T3.
        state[18] = byte as u8;
        state[34] = byte as u8;

        for candidate in candidates(state[], 13) {
          if txc.send(candidate).is_err() {
            panic!("sending failed");
          }
        }
      });
    }

    // There will be ~2^24 entries (minus collisions).
    let total = 256u * 256u * 256u;
    let mut map = HashMap::with_capacity(total);

    // Merge partial results.
    for (key, msg) in rx.iter().take(total) {
      insert(&mut map, key, msg);
    }

    assert!(validate(&map));
    assert_eq!(count(&map), 32784);
  }
}

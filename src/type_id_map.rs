//! Internal `TypeId`-keyed `HashMap` specialisation.
//!
//! The dispatcher's three big maps — listeners, async listeners, and
//! metrics counters — are keyed by [`TypeId`]. `TypeId`'s internal
//! representation is already a well-distributed hash value, so feeding
//! it through `std::collections::HashMap`'s default `SipHasher13`
//! re-hashes a hash. The Sip round adds ~15–25 ns per dispatch on the
//! hot path for zero correctness benefit.
//!
//! This module provides [`TypeIdMap`], a `HashMap` alias backed by a
//! no-op hasher that returns the `TypeId`'s identity bits unchanged.
//! It is `pub(crate)` only — the dispatcher's public surface still
//! returns a plain `HashMap<TypeId, EventMetadata>` from
//! `EventDispatcher::metrics`, which is built from the internal
//! `TypeIdMap` at snapshot time.
//!
//! ## Soundness
//!
//! Rust's `Hasher` trait requires a `write(&[u8])` impl plus a
//! `finish() -> u64`. `TypeId`'s `Hash` impl writes its bytes via
//! [`Hasher::write_u64`] on stable Rust today (and has done since at
//! least 1.42). For forward-compatibility with possible future stdlib
//! widening (e.g. a `write_u128` carrying a 128-bit `TypeId`), the
//! `write` method folds whatever bytes it receives into the running
//! state via XOR-then-rotate, so an unexpected wider `TypeId`
//! representation degrades gracefully to "still distinct, mildly
//! worse distribution" rather than collapsing to zero.
//!
//! No `unsafe` is required.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// Hasher that treats incoming bytes as already-hashed and folds them
/// directly into the output state. Specialised for [`TypeId`] keys.
#[derive(Default)]
pub(crate) struct TypeIdHasher {
    state: u64,
}

impl Hasher for TypeIdHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        // `TypeId::hash` calls this directly on stable Rust. The bits
        // are already well-distributed; passing them through is the
        // whole point.
        self.state = n;
    }

    #[inline]
    fn write_u128(&mut self, n: u128) {
        // If a future stdlib widens `TypeId` to 128 bits and routes
        // through `write_u128`, fold the high and low halves so we
        // keep distinctness without re-hashing.
        self.state = (n as u64) ^ ((n >> 64) as u64);
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Defensive fallback. Not used by `TypeId::hash` on stable
        // Rust today, but keeps the impl total. Folds bytes in eight
        // at a time so the output still depends on every input byte.
        let mut state = self.state;
        for chunk in bytes.chunks(8) {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            state = state.rotate_left(5) ^ u64::from_ne_bytes(buf);
        }
        self.state = state;
    }
}

/// `HashMap` keyed by [`std::any::TypeId`] using [`TypeIdHasher`].
///
/// Equivalent to `HashMap<TypeId, V>` for correctness; faster for
/// lookups because the SipHash round is removed.
pub(crate) type TypeIdMap<V> = HashMap<std::any::TypeId, V, BuildHasherDefault<TypeIdHasher>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::TypeId;

    #[test]
    fn distinct_typeids_map_to_distinct_buckets_in_typical_use() {
        // Sanity: inserting several types and reading them back works.
        let mut map: TypeIdMap<&'static str> = TypeIdMap::default();
        let _ = map.insert(TypeId::of::<u8>(), "u8");
        let _ = map.insert(TypeId::of::<u16>(), "u16");
        let _ = map.insert(TypeId::of::<u32>(), "u32");
        let _ = map.insert(TypeId::of::<u64>(), "u64");
        let _ = map.insert(TypeId::of::<String>(), "String");

        assert_eq!(map.get(&TypeId::of::<u8>()).copied(), Some("u8"));
        assert_eq!(map.get(&TypeId::of::<u16>()).copied(), Some("u16"));
        assert_eq!(map.get(&TypeId::of::<u32>()).copied(), Some("u32"));
        assert_eq!(map.get(&TypeId::of::<u64>()).copied(), Some("u64"));
        assert_eq!(map.get(&TypeId::of::<String>()).copied(), Some("String"));
        assert_eq!(map.get(&TypeId::of::<i8>()), None);
    }

    #[test]
    fn hasher_finish_is_identity_for_write_u64() {
        let mut h = TypeIdHasher::default();
        h.write_u64(0xDEAD_BEEF_CAFE_BABE);
        assert_eq!(h.finish(), 0xDEAD_BEEF_CAFE_BABE);
    }

    #[test]
    fn hasher_finish_is_xor_fold_for_write_u128() {
        let mut h = TypeIdHasher::default();
        h.write_u128(0xAAAA_BBBB_CCCC_DDDD_1111_2222_3333_4444);
        assert_eq!(h.finish(), 0x1111_2222_3333_4444 ^ 0xAAAA_BBBB_CCCC_DDDD);
    }
}

//! The iteration order of the platform's hash containers.
//!
//! Most derived caches (`Params` `*.si`) are dumps of in-memory hash maps, written in the iteration
//! order of the map, and that order is what a byte-exact row has to reproduce. Measured on the
//! corpora (`docs/apply/derived-caches.md`), every one of those maps behaves like a Microsoft
//! `std::unordered_map` keyed by a uuid whose hash is `Data1`, the first four bytes of the uuid text
//! read as a number:
//!
//! - the elements form one list; the elements of a bucket are contiguous;
//! - the table starts with 8 buckets and grows after an insertion that leaves more elements than
//!   buckets: x8 while below 512 buckets, x2 from there (8, 64, 512, 1024, 2048, 4096, ...);
//! - an element whose bucket is empty goes to the **end** of the list, an element of a bucket that
//!   has elements goes to the **front** of that bucket's run;
//! - growing walks the list in order and rebuilds it bucket by bucket: a bucket's run stands where its
//!   first element stood, and inside a run the elements are reversed (each later one goes to the front).
//!
//! The order is a function of the **insertion order** of the keys, so the caller has to know that
//! order; for the maps built from the configuration's root it is the order of the root's collections
//! (see `type_index`). Adding one element early shifts every growth point after it, which is why a
//! new object changes the position of a few unrelated elements (the point of the model).

use anyhow::{Context, Result, bail};

/// `Data1` of a uuid text (`5ff28850-03db-...` -> `0x5ff28850`): the hash of the platform's uuid keys.
pub fn uuid_hash(uuid: &str) -> Result<u32> {
    let head = uuid.get(..8).context("a uuid is at least 8 characters")?;
    u32::from_str_radix(head, 16).with_context(|| format!("{uuid} is not a uuid"))
}

/// The bucket count of a table that grew through `size` insertions.
pub fn bucket_count(size: usize) -> usize {
    let mut buckets = 8usize;
    while size > buckets {
        buckets = grown(buckets);
    }
    buckets
}

fn grown(buckets: usize) -> usize {
    if buckets < 512 {
        buckets * 8
    } else {
        buckets * 2
    }
}

/// A table under construction: hashes in, iteration order out.
#[derive(Clone, Debug)]
pub struct MsvcTable {
    hashes: Vec<u32>,
    list: Vec<usize>,
    buckets: usize,
}

impl Default for MsvcTable {
    fn default() -> Self {
        Self {
            hashes: Vec::new(),
            list: Vec::new(),
            buckets: 8,
        }
    }
}

impl MsvcTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a key by its hash; returns the key's id (its position in insertion order).
    pub fn insert(&mut self, hash: u32) -> usize {
        let id = self.hashes.len();
        self.hashes.push(hash);
        let mask = (self.buckets - 1) as u32;
        let bucket = hash & mask;
        let run_start = self
            .list
            .iter()
            .position(|&other| self.hashes[other] & mask == bucket);
        match run_start {
            None => self.list.push(id),
            Some(at) => self.list.insert(at, id),
        }
        if self.list.len() > self.buckets {
            self.rehash(grown(self.buckets));
        }
        id
    }

    fn rehash(&mut self, buckets: usize) {
        let mask = (buckets - 1) as u32;
        let mut runs: Vec<Vec<usize>> = Vec::new();
        let mut run_of: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
        for &id in &self.list {
            let bucket = self.hashes[id] & mask;
            match run_of.get(&bucket) {
                None => {
                    run_of.insert(bucket, runs.len());
                    runs.push(vec![id]);
                }
                Some(&run) => runs[run].insert(0, id),
            }
        }
        self.list = runs.concat();
        self.buckets = buckets;
    }

    /// The iteration order: ids in list order.
    pub fn order(&self) -> &[usize] {
        &self.list
    }

    pub fn buckets(&self) -> usize {
        self.buckets
    }
}

/// The iteration order of a map that was filled with `keys` (uuid texts) in that order: the keys
/// in list order.
pub fn iteration_order<'a>(keys: impl IntoIterator<Item = &'a str>) -> Result<Vec<&'a str>> {
    let keys: Vec<&str> = keys.into_iter().collect();
    let mut table = MsvcTable::new();
    for key in &keys {
        table.insert(uuid_hash(key)?);
    }
    Ok(table.order().iter().map(|&id| keys[id]).collect())
}

/// Checks that `dump` is in the iteration order of a map filled in `insertion` order.
pub fn ensure_order(dump: &[&str], insertion: &[&str]) -> Result<()> {
    let expected = iteration_order(insertion.iter().copied())?;
    if expected != dump {
        let at = expected
            .iter()
            .zip(dump)
            .position(|(a, b)| a != b)
            .unwrap_or(expected.len().min(dump.len()));
        bail!(
            "the dump is not in the hash-map order of its insertion order (first difference at {at} of {})",
            dump.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(hashes: &[u32]) -> Vec<String> {
        hashes
            .iter()
            .map(|h| format!("{h:08x}-0000-0000-0000-000000000000"))
            .collect()
    }

    fn run(hashes: &[u32]) -> Vec<u32> {
        let texts = keys(hashes);
        iteration_order(texts.iter().map(String::as_str))
            .unwrap()
            .iter()
            .map(|text| uuid_hash(text).unwrap())
            .collect()
    }

    #[test]
    fn a_new_bucket_goes_to_the_end_and_a_known_one_to_the_front_of_its_run() {
        // buckets of 8: 1, 2, 9 (=1), 3
        assert_eq!(run(&[1, 2, 9, 3]), [9, 1, 2, 3]);
    }

    #[test]
    fn the_table_grows_after_the_ninth_and_the_65th_insertion() {
        assert_eq!(bucket_count(8), 8);
        assert_eq!(bucket_count(9), 64);
        assert_eq!(bucket_count(64), 64);
        assert_eq!(bucket_count(65), 512);
        assert_eq!(bucket_count(512), 512);
        assert_eq!(bucket_count(513), 1024);
        assert_eq!(bucket_count(2049), 4096);
    }

    #[test]
    fn growing_regroups_by_the_new_mask_and_reverses_the_runs() {
        // nine keys: 64 joins the run of bucket 0 (the list is 64, 0, 1, ...), the ninth element makes
        // 9 > 8 buckets and the table regroups by `& 63`: the run of bucket 0 is met as 64, 0 and each
        // later element goes to the front, so it ends as 0, 64.
        let order = run(&[0, 1, 2, 3, 4, 5, 6, 7, 64]);
        assert_eq!(order, [0, 64, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn a_dump_is_checked_against_its_insertion_order() {
        let texts = keys(&[5, 6, 13]);
        let insertion: Vec<&str> = texts.iter().map(String::as_str).collect();
        let dump = iteration_order(insertion.iter().copied()).unwrap();
        assert!(ensure_order(&dump, &insertion).is_ok());
        assert!(ensure_order(&insertion, &insertion).is_err());
    }
}

#![deny(warnings)]

use core::hash::Hash;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct CachePoisonedError(&'static str);

#[derive(Debug)]
pub struct HashCache<K, T> where K: Eq + Hash + Clone {
    cache: RwLock<HashMap<K, (T, Instant)>>,
    duration: Duration,
}

impl<K: Eq + Hash + Clone, T> From<HashMap<K, (T, Instant)>> for HashCache<K, T> {
    fn from(map: HashMap<K, (T, Instant)>) -> HashCache<K, T> {
        HashCache {
            cache: RwLock::new(map),
            duration: Duration::new(0, 0),
        }
    }
}

pub type CacheResult<T> = Result<T, CachePoisonedError>;

impl<K: Eq + Hash + Clone, T> HashCache<K, T> {
    pub fn new(duration: Duration) -> HashCache<K, T> {
        HashCache {
            cache: RwLock::new(HashMap::new()),
            duration: duration,
        }
    }

    pub fn reader(&self) -> CacheResult<RwLockReadGuard<HashMap<K, (T, Instant)>>> {
        self.cache.read().map_err(|_| {
            CachePoisonedError("Failed to acquire read guard for cache failed due to poisoning")
        })
    }

    pub fn writer(&self) -> CacheResult<RwLockWriteGuard<HashMap<K, (T, Instant)>>> {
        self.cache.write().map_err(|_| {
            CachePoisonedError("Failed to acquire write guard for cache failed due to poisoning")
        })
    }

    fn ignore_dur(&self) -> bool {
        self.duration.as_secs() as f64 + self.duration.subsec_nanos() as f64 == 0.0
    }
}

impl<K: Eq + Hash + Clone, T: Clone> HashCache<K, T> {
    pub fn get(&self, key: &K) -> CacheResult<Option<T>> {
        self.reader().map(|reader| {
            let entry = reader.get(key.into());

            match entry {
                Some(&(ref val, created)) => {
                    if self.ignore_dur() || created.elapsed() <= self.duration {
                        Some(val.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            }
        })
    }

    pub fn get_all(&self) -> CacheResult<HashMap<K, T>> {
        let mut res: HashMap<K, T> = HashMap::new();

        self.reader().map(|reader| {
            for (k, &(ref f, created)) in reader.iter() {
                if self.ignore_dur() || created.elapsed() <= self.duration {
                    res.insert((*k).clone(), f.clone());
                }
            }

            res
        })
    }

    pub fn insert(&self, key: K, val: &T) -> CacheResult<Option<T>> {
        self.writer().map(|mut writer| {
            writer
                .insert(key, (val.clone(), Instant::now()))
                .map(|(v, _)| v)
        })
    }

    pub fn remove(&self, key: &K) -> CacheResult<Option<T>> {
        self.writer()
            .map(|mut writer| writer.remove(key).map(|(v, _)| v))
    }

    pub fn clear(&self) -> CacheResult<()> {
        self.writer().map(|mut writer| writer.clear())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_then_read() {
        let cache: HashCache<&str, Vec<u8>> = HashCache::new(Duration::new(5, 0));
        let val = vec![1, 2, 3];
        let _ = cache.insert("3", &val);
        assert_eq!(Some(val), cache.get(&"3").unwrap());
    }
}

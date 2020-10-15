// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crossbeam_queue::SegQueue;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Waker;

/// FutureRead module
pub mod read;
/// FutureUpgradableRead module
pub mod upgradable_read;
/// FutureWrite module
pub mod write;

pub use read::FutureReadable;
pub use upgradable_read::{FutureUpgradable, FutureUpgradableReadable};
pub use write::FutureWriteable;

use lock_api::{RawRwLock, RwLock as RwLock_};

use parking_lot::RawRwLock as RawRwLock_;

/// a Future-compatible parking_lot::RwLock
pub type RwLock<T> = RwLock_<FutureRawRwLock<RawRwLock_>, T>;

/// RawRwLock implementor that collects Wakers to wake them up when unlocked
pub struct FutureRawRwLock<R: RawRwLock> {
    locking: AtomicBool,
    wakers: SegQueue<Waker>,
    inner: R,
}

impl<R> FutureRawRwLock<R>
where
    R: RawRwLock,
{
    // this is needed to avoid sequences like that:
    // * thread 1 gains lock
    // * thread 2 try lock
    // * thread 1 unlock
    // * thread 2 register waker
    // this creates a situation similar to a deadlock, where the future isn't waked up by nobody
    fn atomic_lock(&self) {
        while self
            .locking
            .compare_and_swap(false, true, Ordering::Relaxed)
        {}
    }

    fn atomic_unlock(&self) {
        self.locking.store(false, Ordering::Relaxed);
    }

    fn register_waker(&self, waker: &Waker) {
        self.wakers.push(waker.clone());
        // implicitly unlock
        self.atomic_unlock();
    }

    fn wake_up(&self) {
        self.atomic_lock();
        if let Some(w) = self.wakers.pop() {
            w.wake();
        }
        self.atomic_unlock();
    }
}

unsafe impl<R> RawRwLock for FutureRawRwLock<R>
where
    R: RawRwLock,
{
    type GuardMarker = R::GuardMarker;

    const INIT: FutureRawRwLock<R> = {
        FutureRawRwLock {
            locking: AtomicBool::new(false),
            wakers: SegQueue::new(),
            inner: R::INIT,
        }
    };

    fn lock_shared(&self) {
        self.inner.lock_shared();
    }

    fn try_lock_shared(&self) -> bool {
        self.inner.try_lock_shared()
    }

    unsafe fn unlock_shared(&self) {
        self.inner.unlock_shared();

        self.wake_up();
    }

    fn lock_exclusive(&self) {
        self.inner.lock_exclusive();
    }

    fn try_lock_exclusive(&self) -> bool {
        self.inner.try_lock_exclusive()
    }

    unsafe fn unlock_exclusive(&self) {
        self.inner.unlock_exclusive();

        self.wake_up();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::Arc;

    use tokio::runtime::{Builder, Runtime};

    use super::{
        FutureReadable, FutureUpgradable, FutureUpgradableReadable, FutureWriteable, RwLock,
    };

    use lazy_static::lazy_static;

    use log::info;

    lazy_static! {
        static ref LOCK1: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        static ref LOCK2: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        static ref CONCURRENT_LOCK: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
    }

    fn thread_rt() -> Runtime {
        Builder::new().basic_scheduler().build().unwrap()
    }
    fn threadpool_rt() -> Runtime {
        Builder::new().threaded_scheduler().build().unwrap()
    }

    #[test]
    fn current_thread_lazy_static() {
        env_logger::try_init().ok();

        thread_rt().block_on(async {
            {
                let mut v = LOCK1.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = LOCK1.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn current_thread_local_arc() {
        env_logger::try_init().ok();

        let lock = Arc::new(RwLock::new(Vec::new()));
        thread_rt().block_on(async {
            {
                let mut v = lock.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = lock.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn current_thread_local_rc() {
        env_logger::try_init().ok();

        let lock = Rc::new(RwLock::new(Vec::new()));
        thread_rt().block_on(async {
            {
                let mut v = lock.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = lock.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn current_thread_local_box() {
        env_logger::try_init().ok();

        let lock = Box::new(RwLock::new(Vec::new()));
        thread_rt().block_on(async {
            {
                let mut v = lock.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = lock.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_lazy_static() {
        env_logger::try_init().ok();

        threadpool_rt().block_on(async {
            {
                let mut v = LOCK2.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = LOCK2.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_local_arc() {
        env_logger::try_init().ok();

        let lock = Arc::new(RwLock::new(Vec::new()));
        threadpool_rt().block_on(async {
            {
                let mut v = lock.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = lock.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_local_rc() {
        env_logger::try_init().ok();

        let lock = Rc::new(RwLock::new(Vec::new()));
        threadpool_rt().block_on(async {
            {
                let mut v = lock.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = lock.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_local_box() {
        env_logger::try_init().ok();

        let lock = Box::new(RwLock::new(Vec::new()));
        threadpool_rt().block_on(async {
            {
                let mut v = lock.future_write().await;
                v.push(String::from("It works!"));
            }

            let v = lock.future_read().await;
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_concurrent_lazy_static() {
        env_logger::try_init().ok();

        threadpool_rt().block_on(async {
            // spawn 10 concurrent futures
            for i in 0..100 {
                tokio::spawn(async move {
                    {
                        let mut v = CONCURRENT_LOCK.future_write().await;
                        v.push(i.to_string());
                    }

                    let v = CONCURRENT_LOCK.future_read().await;
                    info!("{}, pushed {}", v.len(), i);
                });
            }
        });
        let singleton = CONCURRENT_LOCK.read();
        assert_eq!(singleton.len(), 100);
    }

    #[test]
    fn multithread_concurrent_upgrade() {
        env_logger::try_init().ok();

        let lock = Arc::new(RwLock::new(HashMap::new()));

        threadpool_rt().enter(|| {
            // spawn 10 concurrent futures
            for i in 0usize..100 {
                let lock = lock.clone();
                tokio::spawn(async move {
                    let guard = lock.future_upgradable_read().await;
                    if i % 2 == 0 {
                        let mut guard = FutureUpgradable::future_upgrade(guard).await;
                        guard.insert(i, i);
                    } else {
                        let key = i - 1;
                        let item = guard.get(&key);
                        info!("for key {} found {:?}", key, item);
                        if let Some(&item) = item {
                            assert_eq!(item, key)
                        }
                    }
                });
            }
        });
        let data = lock.read();
        assert_eq!(data.len(), 50);
    }
}

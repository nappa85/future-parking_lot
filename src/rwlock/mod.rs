// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

/// FutureRead module
pub mod read;
/// FutureUpgradableRead module
pub mod upgradable_read;
/// FutureWrite module
pub mod write;

/// Trait to permit FutureRead implementation on wrapped RwLock (not RwLock itself)
pub use read::FutureReadable;
/// Trait to permit FutureUpgradableRead implementation on wrapped RwLock (not RwLock itself)
pub use upgradable_read::FutureUpgradableReadable;
/// Trait to permit FutureWrite implementation on wrapped RwLock (not RwLock itself)
pub use write::FutureWriteable;

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::rc::Rc;

    use tokio::runtime::Runtime as ThreadpoolRuntime;
    use tokio::runtime::current_thread::Runtime as CurrentThreadRuntime;

    use parking_lot::RwLock;

    use super::{FutureReadable, FutureWriteable};

    use lazy_static::lazy_static;

    lazy_static! {
        static ref LOCK1: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        static ref LOCK2: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
        static ref CONCURRENT_LOCK: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
    }

    #[test]
    fn current_thread_lazy_static() {
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
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
        let lock = Arc::new(RwLock::new(Vec::new()));
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
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
        let lock = Rc::new(RwLock::new(Vec::new()));
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
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
        let lock = Box::new(RwLock::new(Vec::new()));
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
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
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
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
        let lock = Arc::new(RwLock::new(Vec::new()));
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
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
        let lock = Rc::new(RwLock::new(Vec::new()));
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
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
        let lock = Box::new(RwLock::new(Vec::new()));
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
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
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
            // spawn 10 concurrent futures
            for i in 0..100 {
                tokio::spawn(async move {
                    {
                        let mut v = CONCURRENT_LOCK.future_write().await;
                        v.push(format!("{}", i));
                    }

                    let v = CONCURRENT_LOCK.future_read().await;
                    println!("{:?}", v);
                });
            }
        });
        runtime.shutdown_on_idle();
        let singleton = CONCURRENT_LOCK.read();
        assert_eq!(singleton.len(), 100);
    }
}

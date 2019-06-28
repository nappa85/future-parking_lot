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

    use tokio::runtime::current_thread;
    use tokio::prelude::{Future, future::lazy};

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
        current_thread::block_on_all(LOCK1.future_write(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            Ok(())
        })
        .and_then(|_| LOCK1.future_read(|v| {
                assert!(v.len() == 1 && v[0] == "It works!");
                Ok(())
        }))).unwrap();
    }

    #[test]
    fn current_thread_local_arc() {
        let lock = Arc::new(RwLock::new(Vec::new()));
        current_thread::block_on_all(lock.future_write(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            Ok(())
        })
        .and_then(|_| lock.future_read(|v| {
                assert!(v.len() == 1 && v[0] == "It works!");
                Ok(())
        }))).unwrap();
    }

    #[test]
    fn current_thread_local_rc() {
        let lock = Rc::new(RwLock::new(Vec::new()));
        current_thread::block_on_all(lock.future_write(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            Ok(())
        })
        .and_then(|_| lock.future_read(|v| {
                assert!(v.len() == 1 && v[0] == "It works!");
                Ok(())
        }))).unwrap();
    }

    #[test]
    fn current_thread_local_box() {
        let lock = Box::new(RwLock::new(Vec::new()));
        current_thread::block_on_all(lock.future_write(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            Ok(())
        })
        .and_then(|_| lock.future_read(|v| {
                assert!(v.len() == 1 && v[0] == "It works!");
                Ok(())
        }))).unwrap();
    }

    #[test]
    fn multithread_lazy_static() {
        tokio::run(LOCK2.future_write(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            Ok(())
        })
        .and_then(|_| LOCK2.future_read(|v| {
                assert!(v.len() == 1 && v[0] == "It works!");
                Ok(())
        })));
    }

    // Implies a lifetime problem
    // #[test]
    // fn multithread_local_arc() {
    //     let lock = Arc::new(RwLock::new(Vec::new()));
    //     tokio::run(lock.future_write(|mut v| {
    //         &v.push(String::from("It works!"));
    //         lock.future_read(|v| {
    //             assert!(v.len() == 1 && v[0] == "It works!");
    //             Ok(())
    //         })
    //     }));
    // }

    // Can't be done because Rc isn't Sync
    // #[test]
    // fn multithread_local_rc() {
    //     let lock = Rc::new(RwLock::new(Vec::new()));
    //     tokio::run(lock.future_write(|mut v| {
    //         &v.push(String::from("It works!"));
    //         lock.future_read(|v| {
    //             assert!(v.len() == 1 && v[0] == "It works!");
    //             Ok(())
    //         })
    //     }));
    // }

    // Implies a lifetime problem
    // #[test]
    // fn multithread_local_box() {
    //     let lock = Box::new(RwLock::new(Vec::new()));
    //     tokio::run(lock.future_write(|mut v| {
    //         &v.push(String::from("It works!"));
    //         lock.future_read(|v| {
    //             assert!(v.len() == 1 && v[0] == "It works!");
    //             Ok(())
    //         })
    //     }));
    // }

    #[test]
    fn multithread_concurrent_lazy_static() {
        tokio::run(lazy(|| {
            // spawn 10 concurrent futures
            for i in 0..100 {
                tokio::spawn(CONCURRENT_LOCK.future_write(move |mut v| {
                    v.push(format!("{}", i));
                    CONCURRENT_LOCK.future_read(|v| {
                        println!("{:?}", v);
                        Ok(())
                    })
                }));
            }
            Ok(())
        }));
        let singleton = CONCURRENT_LOCK.read();
        assert_eq!(singleton.len(), 100);
    }
}

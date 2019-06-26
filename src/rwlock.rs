use std::convert::AsRef;
use std::marker::PhantomData;

use tokio::prelude::{Async, future::{Future, IntoFuture}, task};

use parking_lot::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard, RwLockWriteGuard};

/// Wrapper to read from RwLock in Future-style
pub struct FutureRead<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockReadGuard<'_, T>) -> I,
    I: IntoFuture,
{
    lock: &'a R,
    inner: Option<F>,
    _contents: PhantomData<T>,
    future: Option<I::Future>,
}

impl<'a, R, T, F, I> FutureRead<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockReadGuard<'_, T>) -> I,
    I: IntoFuture,
{
    fn new(lock: &'a R, f: F) -> Self {
        FutureRead {
            lock,
            inner: Some(f),
            _contents: PhantomData,
            future: None,
        }
    }
}

impl<'a, R, T, F, I> Future for FutureRead<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockReadGuard<'_, T>) -> I,
    I: IntoFuture,
{
    type Item = <<I as IntoFuture>::Future as Future>::Item;
    type Error = <<I as IntoFuture>::Future as Future>::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        if let Some(ref mut future) = self.future {
            // Use cached future
            return future.poll();
        }

        match self.lock.as_ref().try_read() {
            Some(read_lock) => {
                // Cache resulting future to avoid executing the inner function again
                let mut future = (self.inner.take().expect("Can't poll on FutureRead more than once"))(read_lock).into_future();
                let res = future.poll();
                self.future = Some(future);
                res
            },
            None => {
                // Notify current Task we can be polled again
                task::current().notify();
                Ok(Async::NotReady)
            },
        }
    }
}

/// Trait to permit FutureRead implementation on wrapped RwLock (not RwLock itself)
pub trait FutureReadable<R: AsRef<RwLock<T>>, T, I: IntoFuture> {
    /// Takes a closure that will be executed when the Futures gains the read-lock
    fn future_read<F: FnOnce(RwLockReadGuard<'_, T>) -> I>(&self, func: F) -> FutureRead<R, T, F, I>;
}

impl<R: AsRef<RwLock<T>>, T, I: IntoFuture> FutureReadable<R, T, I> for R {
    fn future_read<F: FnOnce(RwLockReadGuard<'_, T>) -> I>(&self, func: F) -> FutureRead<R, T, F, I> {
        FutureRead::new(self, func)
    }
}

/// Wrapper to upgradable-read from RwLock in Future-style
pub struct FutureUpgradableRead<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockUpgradableReadGuard<'_, T>) -> I,
    I: IntoFuture,
{
    lock: &'a R,
    inner: Option<F>,
    _contents: PhantomData<T>,
    future: Option<I::Future>,
}

impl<'a, R, T, F, I> FutureUpgradableRead<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockUpgradableReadGuard<'_, T>) -> I,
    I: IntoFuture,
{
    fn new(lock: &'a R, f: F) -> Self {
        FutureUpgradableRead {
            lock,
            inner: Some(f),
            _contents: PhantomData,
            future: None,
        }
    }
}

impl<'a, R, T, F, I> Future for FutureUpgradableRead<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockUpgradableReadGuard<'_, T>) -> I,
    I: IntoFuture,
{
    type Item = <<I as IntoFuture>::Future as Future>::Item;
    type Error = <<I as IntoFuture>::Future as Future>::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        if let Some(ref mut future) = self.future {
            // Use cached future
            return future.poll();
        }

        match self.lock.as_ref().try_upgradable_read() {
            Some(upgradable_lock) => {
                // Cache resulting future to avoid executing the inner function again
                let mut future = (self.inner.take().expect("Can't poll on FutureUpgradableRead more than once"))(upgradable_lock).into_future();
                let res = future.poll();
                self.future = Some(future);
                res
            },
            None => {
                // Notify current Task we can be polled again
                task::current().notify();
                Ok(Async::NotReady)
            },
        }
    }
}

/// Trait to permit FutureUpgradableRead implementation on wrapped RwLock (not RwLock itself)
pub trait FutureUpgradableReadable<R: AsRef<RwLock<T>>, T, I: IntoFuture> {
    /// Takes a closure that will be executed when the Futures gains the upgradable-read-lock
    fn future_upgradable_read<F: FnOnce(RwLockUpgradableReadGuard<'_, T>) -> I>(&self, func: F) -> FutureUpgradableRead<R, T, F, I>;
}

impl<R: AsRef<RwLock<T>>, T, I: IntoFuture> FutureUpgradableReadable<R, T, I> for R {
    fn future_upgradable_read<F: FnOnce(RwLockUpgradableReadGuard<'_, T>) -> I>(&self, func: F) -> FutureUpgradableRead<R, T, F, I> {
        FutureUpgradableRead::new(self, func)
    }
}

/// Wrapper to write into RwLock in Future-style
pub struct FutureWrite<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockWriteGuard<'_, T>) -> I,
    I: IntoFuture,
{
    lock: &'a R,
    inner: Option<F>,
    _contents: PhantomData<T>,
    future: Option<I::Future>,
}

impl<'a, R, T, F, I> FutureWrite<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockWriteGuard<'_, T>) -> I,
    I: IntoFuture,
{
    fn new(lock: &'a R, f: F) -> Self {
        FutureWrite {
            lock,
            inner: Some(f),
            _contents: PhantomData,
            future: None,
        }
    }
}

impl<'a, R, T, F, I> Future for FutureWrite<'a, R, T, F, I>
where
    R: AsRef<RwLock<T>>,
    F: FnOnce(RwLockWriteGuard<'_, T>) -> I,
    I: IntoFuture,
{
    type Item = <<I as IntoFuture>::Future as Future>::Item;
    type Error = <<I as IntoFuture>::Future as Future>::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        if let Some(ref mut future) = self.future {
            // Use cached future
            return future.poll();
        }

        match self.lock.as_ref().try_write() {
            Some(write_lock) => {
                // Cache resulting future to avoid executing the inner function again
                let mut future = (self.inner.take().expect("Can't poll on FutureWrite more than once"))(write_lock).into_future();
                let res = future.poll();
                self.future = Some(future);
                res
            },
            None => {
                // Notify current Task we can be polled again
                task::current().notify();
                Ok(Async::NotReady)
            },
        }
    }
}

/// Trait to permit FutureWrite implementation on wrapped RwLock (not RwLock itself)
pub trait FutureWriteable<R: AsRef<RwLock<T>>, T, I: IntoFuture> {
    /// Takes a closure that will be executed when the Futures gains the write-lock
    fn future_write<F: FnOnce(RwLockWriteGuard<'_, T>) -> I>(&self, func: F) -> FutureWrite<R, T, F, I>;
}

impl<R: AsRef<RwLock<T>>, T, I: IntoFuture> FutureWriteable<R, T, I> for R {
    fn future_write<F: FnOnce(RwLockWriteGuard<'_, T>) -> I>(&self, func: F) -> FutureWrite<R, T, F, I> {
        FutureWrite::new(self, func)
    }
}

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

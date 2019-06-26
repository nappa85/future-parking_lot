use std::convert::AsRef;
use std::marker::PhantomData;

use tokio::prelude::{Async, future::{Future, IntoFuture}, task};

use parking_lot::{Mutex, MutexGuard};

/// Wrapper to use Mutex in Future-style
pub struct FutureLock<'a, R, T, F, I>
where
    R: AsRef<Mutex<T>>,
    F: FnOnce(MutexGuard<'_, T>) -> I,
    I: IntoFuture,
{
    lock: &'a R,
    inner: Option<F>,
    _contents: PhantomData<T>,
    future: Option<I::Future>,
}

impl<'a, R, T, F, I> FutureLock<'a, R, T, F, I>
where
    R: AsRef<Mutex<T>>,
    F: FnOnce(MutexGuard<'_, T>) -> I,
    I: IntoFuture,
{
    fn new(lock: &'a R, f: F) -> Self {
        FutureLock {
            lock,
            inner: Some(f),
            _contents: PhantomData,
            future: None,
        }
    }
}

impl<'a, R, T, F, I> Future for FutureLock<'a, R, T, F, I>
where
    R: AsRef<Mutex<T>>,
    F: FnOnce(MutexGuard<'_, T>) -> I,
    I: IntoFuture,
{
    type Item = <<I as IntoFuture>::Future as Future>::Item;
    type Error = <<I as IntoFuture>::Future as Future>::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        if let Some(ref mut future) = self.future {
            // Use cached future
            return future.poll();
        }

        match self.lock.as_ref().try_lock() {
            Some(read_lock) => {
                // Cache resulting future to avoid executing the inner function again
                let mut future = (self.inner.take().expect("Can't poll on FutureLock more than once"))(read_lock).into_future();
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

/// Trait to permit FutureLock implementation on wrapped Mutex (not Mutex itself)
pub trait FutureLockable<R: AsRef<Mutex<T>>, T, I: IntoFuture> {
    /// Takes a closure that will be executed when the Futures gains the read-lock
    fn future_lock<F: FnOnce(MutexGuard<'_, T>) -> I>(&self, func: F) -> FutureLock<R, T, F, I>;
}

impl<R: AsRef<Mutex<T>>, T, I: IntoFuture> FutureLockable<R, T, I> for R {
    fn future_lock<F: FnOnce(MutexGuard<'_, T>) -> I>(&self, func: F) -> FutureLock<R, T, F, I> {
        FutureLock::new(self, func)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::rc::Rc;

    use tokio::runtime::current_thread;
    use tokio::prelude::future::lazy;

    use parking_lot::Mutex;

    use super::{FutureLockable};

    use lazy_static::lazy_static;

    lazy_static! {
        static ref LOCK1: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        static ref LOCK2: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        static ref CONCURRENT_LOCK: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    }

    #[test]
    fn current_thread_lazy_static() {
        current_thread::block_on_all(LOCK1.future_lock(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
            Ok(())
        })).unwrap();
    }

    #[test]
    fn current_thread_local_arc() {
        let lock = Arc::new(Mutex::new(Vec::new()));
        current_thread::block_on_all(lock.future_lock(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
            Ok(())
        })).unwrap();
    }

    #[test]
    fn current_thread_local_rc() {
        let lock = Rc::new(Mutex::new(Vec::new()));
        current_thread::block_on_all(lock.future_lock(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
            Ok(())
        })).unwrap();
    }

    #[test]
    fn current_thread_local_box() {
        let lock = Box::new(Mutex::new(Vec::new()));
        current_thread::block_on_all(lock.future_lock(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
            Ok(())
        })).unwrap();
    }

    #[test]
    fn multithread_lazy_static() {
        tokio::run(LOCK2.future_lock(|mut v| -> Result<(), ()> {
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
            Ok(())
        }));
    }

    // Implies a lifetime problem
    // #[test]
    // fn multithread_local_arc() {
    //     let lock = Arc::new(Mutex::new(Vec::new()));
    //     tokio::run(lock.future_lock(|mut v| {
    //         v.push(String::from("It works!"));
    //         assert!(v.len() == 1 && v[0] == "It works!");
    //         Ok(())
    //     });
    // }

    // Can't be done because Rc isn't Sync
    // #[test]
    // fn multithread_local_rc() {
    //     let lock = Rc::new(Mutex::new(Vec::new()));
    //     tokio::run(lock.future_lock(|mut v| {
    //         v.push(String::from("It works!"));
    //         assert!(v.len() == 1 && v[0] == "It works!");
    //         Ok(())
    //     });
    // }

    // Implies a lifetime problem
    // #[test]
    // fn multithread_local_box() {
    //     let lock = Box::new(Mutex::new(Vec::new()));
    //     tokio::run(lock.future_lock(|mut v| {
    //         v.push(String::from("It works!"));
    //         assert!(v.len() == 1 && v[0] == "It works!");
    //         Ok(())
    //     });
    // }

    #[test]
    fn multithread_concurrent_lazy_static() {
        tokio::run(lazy(|| {
            // spawn 10 concurrent futures
            for i in 0..100 {
                tokio::spawn(CONCURRENT_LOCK.future_lock(move |mut v| {
                    v.push(format!("{}", i));
                    println!("{:?}", v);
                    Ok(())
                }));
            }
            Ok(())
        }));
        let singleton = CONCURRENT_LOCK.lock();
        assert_eq!(singleton.len(), 100);
    }
}

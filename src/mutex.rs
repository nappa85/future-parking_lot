// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use crossbeam_queue::SegQueue;
use once_cell::sync::Lazy;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use lock_api::{Mutex as Mutex_, MutexGuard, RawMutex};

use parking_lot::RawMutex as RawMutex_;

/// a Future-compatible parking_lot::Mutex
pub type Mutex<T> = Mutex_<FutureRawMutex<RawMutex_>, T>;

/// RawMutex implementor that collects Wakers to wake them up when unlocked
pub struct FutureRawMutex<R>
where
    R: RawMutex,
{
    locking: AtomicBool,
    wakers: Lazy<SegQueue<Waker>>,
    inner: R,
}

impl<R> FutureRawMutex<R>
where
    R: RawMutex,
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
        if let Ok(w) = self.wakers.pop() {
            w.wake();
        }
        self.atomic_unlock();
    }
}

unsafe impl<R> RawMutex for FutureRawMutex<R>
where
    R: RawMutex,
{
    type GuardMarker = R::GuardMarker;

    const INIT: FutureRawMutex<R> = {
        FutureRawMutex {
            locking: AtomicBool::new(false),
            wakers: Lazy::new(SegQueue::new),
            inner: R::INIT,
        }
    };

    fn lock(&self) {
        self.inner.lock();
    }

    fn try_lock(&self) -> bool {
        self.inner.try_lock()
    }

    fn unlock(&self) {
        self.inner.unlock();

        self.wake_up();
    }
}

/// Wrapper to use Mutex in Future-style
pub struct FutureLock<'a, R, T>
where
    R: RawMutex + 'a,
    T: 'a,
{
    lock: &'a Mutex_<FutureRawMutex<R>, T>,
    _contents: PhantomData<T>,
    _locktype: PhantomData<R>,
}

impl<'a, R, T> FutureLock<'a, R, T>
where
    R: RawMutex + 'a,
    T: 'a,
{
    fn new(lock: &'a Mutex_<FutureRawMutex<R>, T>) -> Self {
        FutureLock {
            lock,
            _contents: PhantomData,
            _locktype: PhantomData,
        }
    }
}

impl<'a, R, T> Future for FutureLock<'a, R, T>
where
    R: RawMutex + 'a,
    T: 'a,
{
    type Output = MutexGuard<'a, FutureRawMutex<R>, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe {
            self.lock.raw().atomic_lock();
        }
        match self.lock.try_lock() {
            Some(read_lock) => {
                unsafe {
                    self.lock.raw().atomic_unlock();
                }
                Poll::Ready(read_lock)
            }
            None => {
                // Register Waker so we can notified when we can be polled again
                unsafe {
                    self.lock.raw().register_waker(cx.waker());
                }
                Poll::Pending
            }
        }
    }
}

/// Trait to permit FutureLock implementation on wrapped Mutex (not Mutex itself)
pub trait FutureLockable<R: RawMutex, T> {
    /// Returns the lock without blocking
    fn future_lock(&self) -> FutureLock<R, T>;
}

impl<R: RawMutex, T> FutureLockable<R, T> for Mutex_<FutureRawMutex<R>, T> {
    fn future_lock(&self) -> FutureLock<R, T> {
        FutureLock::new(self)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use tokio::runtime::current_thread::Runtime as CurrentThreadRuntime;
    use tokio::runtime::Runtime as ThreadpoolRuntime;

    use super::Mutex;

    use super::FutureLockable;

    use lazy_static::lazy_static;

    use log::debug;

    lazy_static! {
        static ref LOCK1: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        static ref LOCK2: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        static ref CONCURRENT_LOCK: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    }

    #[test]
    fn current_thread_lazy_static() {
        env_logger::try_init().ok();

        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = LOCK1.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn current_thread_local_arc() {
        env_logger::try_init().ok();

        let lock = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = lock.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn current_thread_local_rc() {
        env_logger::try_init().ok();

        let lock = Rc::new(Mutex::new(Vec::new()));
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = lock.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn current_thread_local_box() {
        env_logger::try_init().ok();

        let lock = Box::new(Mutex::new(Vec::new()));
        let mut runtime = CurrentThreadRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = lock.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_lazy_static() {
        env_logger::try_init().ok();

        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = LOCK2.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_local_arc() {
        env_logger::try_init().ok();

        let lock = Arc::new(Mutex::new(Vec::new()));
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = lock.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_local_rc() {
        env_logger::try_init().ok();

        let lock = Rc::new(Mutex::new(Vec::new()));
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = lock.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_local_box() {
        env_logger::try_init().ok();

        let lock = Box::new(Mutex::new(Vec::new()));
        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
            let mut v = lock.future_lock().await;
            v.push(String::from("It works!"));
            assert!(v.len() == 1 && v[0] == "It works!");
        });
    }

    #[test]
    fn multithread_concurrent_lazy_static() {
        env_logger::try_init().ok();

        let runtime = ThreadpoolRuntime::new().unwrap();
        runtime.block_on(async {
            // spawn 1000 concurrent futures
            for i in 0..1000 {
                tokio::spawn(async move {
                    let mut v = CONCURRENT_LOCK.future_lock().await;
                    v.push(i.to_string());
                    debug!("{}, pushed {}", v.len(), i);
                });
            }
        });
        runtime.shutdown_on_idle();
        let singleton = CONCURRENT_LOCK.lock();
        assert_eq!(singleton.len(), 1000);
    }
}

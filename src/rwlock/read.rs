// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use lock_api::{RawRwLock, RwLock, RwLockReadGuard};

use super::FutureRawRwLock;

/// Wrapper to read from RwLock in Future-style
pub struct FutureRead<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    lock: &'a RwLock<FutureRawRwLock<R>, T>,
}

impl<'a, R, T> FutureRead<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    fn new(lock: &'a RwLock<FutureRawRwLock<R>, T>) -> Self {
        FutureRead { lock }
    }
}

impl<'a, R, T> Future for FutureRead<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    type Output = RwLockReadGuard<'a, FutureRawRwLock<R>, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe {
            self.lock.raw().atomic_lock();
        }
        match self.lock.try_read() {
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

/// Trait to permit FutureRead implementation on wrapped RwLock (not RwLock itself)
pub trait FutureReadable<R: RawRwLock, T> {
    /// Returns the read-lock without blocking
    fn future_read(&self) -> FutureRead<R, T>;
}

impl<R: RawRwLock, T> FutureReadable<R, T> for RwLock<FutureRawRwLock<R>, T> {
    fn future_read(&self) -> FutureRead<R, T> {
        FutureRead::new(self)
    }
}

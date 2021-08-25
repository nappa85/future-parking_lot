// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use lock_api::{RawRwLock, RwLock, RwLockWriteGuard};

use super::FutureRawRwLock;

/// Wrapper to write into RwLock in Future-style
pub struct FutureWrite<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    lock: &'a RwLock<FutureRawRwLock<R>, T>,
}

impl<'a, R, T> FutureWrite<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    fn new(lock: &'a RwLock<FutureRawRwLock<R>, T>) -> Self {
        FutureWrite { lock }
    }
}

impl<'a, R, T> Future for FutureWrite<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    type Output = RwLockWriteGuard<'a, FutureRawRwLock<R>, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe {
            self.lock.raw().atomic_lock();
        }
        match self.lock.try_write() {
            Some(write_lock) => {
                unsafe {
                    self.lock.raw().atomic_unlock();
                }
                Poll::Ready(write_lock)
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

/// Trait to permit FutureWrite implementation on wrapped RwLock (not RwLock itself)
pub trait FutureWriteable<R: RawRwLock, T> {
    /// Returns the write-lock without blocking
    fn future_write(&self) -> FutureWrite<R, T>;
}

impl<R: RawRwLock, T> FutureWriteable<R, T> for RwLock<FutureRawRwLock<R>, T> {
    fn future_write(&self) -> FutureWrite<R, T> {
        FutureWrite::new(self)
    }
}

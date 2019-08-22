// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::marker::PhantomData;
use std::future::Future;
use std::task::{Poll, Context};
use std::pin::Pin;

// use future_utils::*;

use lock_api::{RwLock, RawRwLock, RwLockWriteGuard};

/// Wrapper to write into RwLock in Future-style
pub struct FutureWrite<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    lock: &'a RwLock<R, T>,
    _contents: PhantomData<T>,
    _locktype: PhantomData<R>,
}

impl<'a, R, T> FutureWrite<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    fn new(lock: &'a RwLock<R, T>) -> Self {
        FutureWrite {
            lock,
            _contents: PhantomData,
            _locktype: PhantomData,
        }
    }
}

impl<'a, R, T> Future for FutureWrite<'a, R, T>
where
    R: RawRwLock + 'a,
    T: 'a,
{
    type Output = RwLockWriteGuard<'a, R, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.lock.try_write() {
            Some(write_lock) => Poll::Ready(write_lock),
            None => {
                // Notify current Task we can be polled again
                cx.waker().wake_by_ref();
                Poll::Pending
            },
        }
    }
}

/// Trait to permit FutureWrite implementation on wrapped RwLock (not RwLock itself)
pub trait FutureWriteable<R: RawRwLock, T> {
    /// Returns the write-lock without blocking
    fn future_write(&self) -> FutureWrite<R, T>;
}

impl<R: RawRwLock, T> FutureWriteable<R, T> for RwLock<R, T> {
    fn future_write(&self) -> FutureWrite<R, T> {
        FutureWrite::new(self)
    }
}

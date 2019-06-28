// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use std::convert::AsRef;
use std::marker::PhantomData;

use tokio::prelude::{Async, future::{Future, IntoFuture}, task};

use lock_api::{RwLock, RawRwLock, RwLockWriteGuard};

/// Wrapper to write into RwLock in Future-style
pub struct FutureWrite<'a, L, R, T, F, I>
where
    L: AsRef<RwLock<R, T>>,
    R: RawRwLock,
    F: FnOnce(RwLockWriteGuard<'_, R, T>) -> I,
    I: IntoFuture,
{
    lock: &'a L,
    inner: Option<F>,
    _contents: PhantomData<T>,
    _locktype: PhantomData<R>,
    future: Option<I::Future>,
}

impl<'a, L, R, T, F, I> FutureWrite<'a, L, R, T, F, I>
where
    L: AsRef<RwLock<R, T>>,
    R: RawRwLock,
    F: FnOnce(RwLockWriteGuard<'_, R, T>) -> I,
    I: IntoFuture,
{
    fn new(lock: &'a L, f: F) -> Self {
        FutureWrite {
            lock,
            inner: Some(f),
            _contents: PhantomData,
            _locktype: PhantomData,
            future: None,
        }
    }
}

impl<'a, L, R, T, F, I> Future for FutureWrite<'a, L, R, T, F, I>
where
    L: AsRef<RwLock<R, T>>,
    R: RawRwLock,
    F: FnOnce(RwLockWriteGuard<'_, R, T>) -> I,
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
pub trait FutureWriteable<L: AsRef<RwLock<R, T>>, R: RawRwLock, T, I: IntoFuture> {
    /// Takes a closure that will be executed when the Futures gains the write-lock
    fn future_write<F: FnOnce(RwLockWriteGuard<'_, R, T>) -> I>(&self, func: F) -> FutureWrite<L, R, T, F, I>;
}

impl<L: AsRef<RwLock<R, T>>, R: RawRwLock, T, I: IntoFuture> FutureWriteable<L, R, T, I> for L {
    fn future_write<F: FnOnce(RwLockWriteGuard<'_, R, T>) -> I>(&self, func: F) -> FutureWrite<L, R, T, F, I> {
        FutureWrite::new(self, func)
    }
}

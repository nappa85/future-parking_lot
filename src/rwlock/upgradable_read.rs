// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
use std::convert::AsRef;
use std::marker::PhantomData;

use tokio::prelude::{Async, future::{Future, IntoFuture}, task};

use lock_api::{RwLock, RawRwLockUpgrade, RwLockUpgradableReadGuard};

/// Wrapper to upgradable-read from RwLock in Future-style
pub struct FutureUpgradableRead<'a, L, R, T, F, I>
where
    L: AsRef<RwLock<R, T>>,
    R: RawRwLockUpgrade,
    F: FnOnce(RwLockUpgradableReadGuard<'_, R, T>) -> I,
    I: IntoFuture,
{
    lock: &'a L,
    inner: Option<F>,
    _contents: PhantomData<T>,
    _locktype: PhantomData<R>,
    future: Option<I::Future>,
}

impl<'a, L, R, T, F, I> FutureUpgradableRead<'a, L, R, T, F, I>
where
    L: AsRef<RwLock<R, T>>,
    R: RawRwLockUpgrade,
    F: FnOnce(RwLockUpgradableReadGuard<'_, R, T>) -> I,
    I: IntoFuture,
{
    fn new(lock: &'a L, f: F) -> Self {
        FutureUpgradableRead {
            lock,
            inner: Some(f),
            _contents: PhantomData,
            _locktype: PhantomData,
            future: None,
        }
    }
}

impl<'a, L, R, T, F, I> Future for FutureUpgradableRead<'a, L, R, T, F, I>
where
    L: AsRef<RwLock<R, T>>,
    R: RawRwLockUpgrade,
    F: FnOnce(RwLockUpgradableReadGuard<'_, R, T>) -> I,
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
pub trait FutureUpgradableReadable<L: AsRef<RwLock<R, T>>, R: RawRwLockUpgrade, T, I: IntoFuture> {
    /// Takes a closure that will be executed when the Futures gains the upgradable-read-lock
    fn future_upgradable_read<F: FnOnce(RwLockUpgradableReadGuard<'_, R, T>) -> I>(&self, func: F) -> FutureUpgradableRead<L, R, T, F, I>;
}

impl<L: AsRef<RwLock<R, T>>, R: RawRwLockUpgrade, T, I: IntoFuture> FutureUpgradableReadable<L, R, T, I> for L {
    fn future_upgradable_read<F: FnOnce(RwLockUpgradableReadGuard<'_, R, T>) -> I>(&self, func: F) -> FutureUpgradableRead<L, R, T, F, I> {
        FutureUpgradableRead::new(self, func)
    }
}

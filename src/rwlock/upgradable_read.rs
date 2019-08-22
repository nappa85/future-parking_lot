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

use lock_api::{RwLock, RawRwLockUpgrade, RwLockUpgradableReadGuard};

/// Wrapper to upgradable-read from RwLock in Future-style
pub struct FutureUpgradableRead<'a, R, T>
where
    R: RawRwLockUpgrade + 'a,
    T: 'a,
{
    lock: &'a RwLock<R, T>,
    _contents: PhantomData<T>,
    _locktype: PhantomData<R>,
}

impl<'a, R, T> FutureUpgradableRead<'a, R, T>
where
    R: RawRwLockUpgrade + 'a,
    T: 'a,
{
    fn new(lock: &'a RwLock<R, T>) -> Self {
        FutureUpgradableRead {
            lock,
            _contents: PhantomData,
            _locktype: PhantomData,
        }
    }
}

impl<'a, R, T> Future for FutureUpgradableRead<'a, R, T>
where
    R: RawRwLockUpgrade + 'a,
    T: 'a,
{
    type Output = RwLockUpgradableReadGuard<'a, R, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        match self.lock.try_upgradable_read() {
            Some(upgradable_lock) => Poll::Ready(upgradable_lock),
            None => {
                // Notify current Task we can be polled again
                cx.waker().wake_by_ref();
                Poll::Pending
            },
        }
    }
}

/// Trait to permit FutureUpgradableRead implementation on wrapped RwLock (not RwLock itself)
pub trait FutureUpgradableReadable<R: RawRwLockUpgrade, T> {
    /// Returns the upgradable-read-lock without blocking
    fn future_upgradable_read(&self) -> FutureUpgradableRead<R, T>;
}

impl<R: RawRwLockUpgrade, T> FutureUpgradableReadable<R, T> for RwLock<R, T> {
    fn future_upgradable_read(&self) -> FutureUpgradableRead<R, T> {
        FutureUpgradableRead::new(self)
    }
}

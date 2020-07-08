// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use lock_api::{RawRwLockUpgrade, RwLock, RwLockUpgradableReadGuard};

use super::FutureRawRwLock;

unsafe impl<R> RawRwLockUpgrade for FutureRawRwLock<R>
where
    R: RawRwLockUpgrade,
{
    fn lock_upgradable(&self) {
        self.inner.lock_upgradable();
    }

    fn try_lock_upgradable(&self) -> bool {
        self.inner.try_lock_upgradable()
    }

    unsafe fn unlock_upgradable(&self) {
        self.inner.unlock_upgradable();

        self.wake_up();
    }

    unsafe fn upgrade(&self) {
        self.inner.upgrade();
    }

    unsafe fn try_upgrade(&self) -> bool {
        self.inner.try_upgrade()
    }
}

/// Wrapper to upgradable-read from RwLock in Future-style
pub struct FutureUpgradableRead<'a, R, T>
where
    R: RawRwLockUpgrade + 'a,
    T: 'a,
{
    lock: &'a RwLock<FutureRawRwLock<R>, T>,
    _contents: PhantomData<T>,
    _locktype: PhantomData<R>,
}

impl<'a, R, T> FutureUpgradableRead<'a, R, T>
where
    R: RawRwLockUpgrade + 'a,
    T: 'a,
{
    fn new(lock: &'a RwLock<FutureRawRwLock<R>, T>) -> Self {
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
    type Output = RwLockUpgradableReadGuard<'a, FutureRawRwLock<R>, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        unsafe {
            self.lock.raw().atomic_lock();
        }
        match self.lock.try_upgradable_read() {
            Some(upgradable_lock) => {
                unsafe {
                    self.lock.raw().atomic_unlock();
                }
                Poll::Ready(upgradable_lock)
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

/// Trait to permit FutureUpgradableRead implementation on wrapped RwLock (not RwLock itself)
pub trait FutureUpgradableReadable<R: RawRwLockUpgrade, T> {
    /// Returns the upgradable-read-lock without blocking
    fn future_upgradable_read(&self) -> FutureUpgradableRead<R, T>;
}

impl<R: RawRwLockUpgrade, T> FutureUpgradableReadable<R, T> for RwLock<FutureRawRwLock<R>, T> {
    fn future_upgradable_read(&self) -> FutureUpgradableRead<R, T> {
        FutureUpgradableRead::new(self)
    }
}

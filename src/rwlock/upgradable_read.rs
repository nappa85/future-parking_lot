// Copyright 2018 Marco Napetti
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use lock_api::{RawRwLockUpgrade, RwLock, RwLockUpgradableReadGuard, RwLockWriteGuard};

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
    R: RawRwLockUpgrade,
{
    lock: &'a RwLock<FutureRawRwLock<R>, T>,
}

impl<'a, R, T> FutureUpgradableRead<'a, R, T>
where
    R: RawRwLockUpgrade + 'a,
    T: 'a,
{
    fn new(lock: &'a RwLock<FutureRawRwLock<R>, T>) -> Self {
        FutureUpgradableRead { lock }
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

pin_project_lite::pin_project! {
    /// Wrapper to upgrade a RwLockUpgradableReadGuard in Future-style
    pub struct FutureUpgrade<'a, R, T>
    where
        R: RawRwLockUpgrade,
        T: ?Sized,
    {
        rlock: Option<RwLockUpgradableReadGuard<'a, FutureRawRwLock<R>, T>>,
    }
}

impl<'a, R: RawRwLockUpgrade, T: ?Sized> Future for FutureUpgrade<'a, R, T> {
    type Output = RwLockWriteGuard<'a, FutureRawRwLock<R>, T>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let rlock = self
            .rlock
            .take()
            .expect("can't poll FutureUpgrade after completed");
        unsafe {
            RwLockUpgradableReadGuard::rwlock(&rlock)
                .raw()
                .atomic_lock();
        }
        match RwLockUpgradableReadGuard::try_upgrade(rlock) {
            Ok(wlock) => {
                unsafe {
                    RwLockWriteGuard::rwlock(&wlock).raw().atomic_unlock();
                }
                Poll::Ready(wlock)
            }
            Err(rlock) => {
                // Register Waker so we can notified when we can be polled again
                unsafe {
                    RwLockUpgradableReadGuard::rwlock(&rlock)
                        .raw()
                        .register_waker(cx.waker());
                }
                self.rlock = Some(rlock);
                Poll::Pending
            }
        }
    }
}

/// Trait to permit FutureUpgrade implementation on wrapped RwLock (not RwLock itself)
pub trait FutureUpgradable<'a, R: RawRwLockUpgrade, T: ?Sized> {
    /// Returns the upgraded lock without blocking
    fn future_upgrade(s: Self) -> FutureUpgrade<'a, R, T>;
}

impl<'a, R: RawRwLockUpgrade, T: ?Sized> FutureUpgradable<'a, R, T>
    for RwLockUpgradableReadGuard<'a, FutureRawRwLock<R>, T>
{
    fn future_upgrade(s: Self) -> FutureUpgrade<'a, R, T> {
        FutureUpgrade { rlock: Some(s) }
    }
}

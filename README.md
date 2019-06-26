# future-parking_lot

This is an "as simple as possible" Future implementation for [parking_lot](https://docs.rs/parking_lot/0.8.0/parking_lot/).
It works on `AsRef<Mutex<T>>` and `AsRef<RwLock<T>>` instead of directly on `Mutex<T>` and `RwLock<T>` for borrowing reasons (the lock must not live inside the Future, it would be dropped with it).
This comes handy when you have global vars (like when using lazy_static) and you need to edit them from a Future environment.

Example:
```
use std::sync::Arc;

use tokio::run;

use parking_lot::RwLock;

use future_parking_lot::rwlock::{FutureReadable, FutureWriteable};

use lazy_static::lazy_static;

lazy_static! {
    static ref LOCK: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
}

fn main() {
    run(LOCK.future_write(|mut v| {
        v.push(String::from("It works!"));
        LOCK.future_read(|v| -> Result<(), ()> {
            assert!(v.len() == 1 && v[0] == "It works!");
            Ok(())
        })
    }));
}
```

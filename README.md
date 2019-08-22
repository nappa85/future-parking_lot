# future-parking_lot

This is an "as simple as possible" Future implementation for [parking_lot](https://docs.rs/parking_lot/0.9.0/parking_lot/).
Thanks to async/await feature, now it works directly on `Mutex<T>` and `RwLock<T>`.

Example:
```
use std::sync::Arc;

use parking_lot::RwLock;

use future_parking_lot::rwlock::{FutureReadable, FutureWriteable};

use lazy_static::lazy_static;

lazy_static! {
    static ref LOCK: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    {
        let mut v = LOCK.future_write().await;
        v.push(String::from("It works!"));
    }

    let v = LOCK.future_read().await;
    assert!(v.len() == 1 && v[0] == "It works!");

    Ok(())
}
```

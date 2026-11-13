use crate::result::Result;

use core::cell::SyncUnsafeCell;
use core::fmt::Debug;
use core::ops::Deref;
use core::ops::DerefMut;
use core::panic::Location;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering;

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
    data: &'a mut T,
    location: Location<'a>,
}

impl<'a, T> MutexGuard<'a, T> {

    #[track_caller]
    unsafe fn new(mutex: &'a Mutex<T>, data: &SyncUnsafeCell<T>) -> Self {
        
    }

}
#![allow(unused)]

use std::ops::{Deref, DerefMut};

pub type RawHandle = u64;

#[repr(transparent)]
#[derive(Debug, PartialEq, Eq)]
pub struct Handle<T>(*mut T);

impl<T> Handle<T> {
    pub fn new(v: T) -> Self {
        let handle = Box::new(v);
        Handle(Box::into_raw(handle))
    }

    pub fn raw(self) -> RawHandle {
        self.0 as _
    }

    pub fn from_raw(handle: RawHandle) -> Self {
        Handle(handle as _)
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle(self.0)
    }
}

impl<T> Deref for Handle<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T> DerefMut for Handle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

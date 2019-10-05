// use std::ops::Deref;

pub type RawHandle = u64;

// #[repr(transparent)]
// #[derive(Debug, Copy, Clone)]
// pub struct Handle<T>(*mut T);

// impl<T> Handle<T> {
//     pub fn new(v: T) -> Self {
//         let handle = Box::new(v);
//         Handle(Box::into_raw(handle))
//     }

//     pub fn raw(self) -> RawHandle {
//         self.0 as _
//     }

//     pub fn from_raw(handle: RawHandle) -> Self {
//         Handle(handle as _)
//     }
// }

// impl<T> Deref for Handle<T> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         unsafe { &*self.0 }
//     }
// }

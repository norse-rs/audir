use std::ptr;
use winapi::um::{handleapi, synchapi, winnt};

#[derive(Copy, Clone)]
pub struct Fence(pub winnt::HANDLE);
impl Fence {
    pub unsafe fn create(manual_reset: bool, initial_state: bool) -> Self {
        Fence(synchapi::CreateEventA(
            ptr::null_mut(),
            manual_reset as _,
            initial_state as _,
            ptr::null(),
        ))
    }

    pub unsafe fn destory(self) {
        handleapi::CloseHandle(self.0);
    }

    pub unsafe fn wait(&self, timeout_ms: u32) -> u32 {
        synchapi::WaitForSingleObject(self.0, timeout_ms)
    }
}

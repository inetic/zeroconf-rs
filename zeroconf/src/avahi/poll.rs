//! Rust friendly `AvahiSimplePoll` wrappers/helpers

use crate::Result;
use crate::{avahi::avahi_util, error::Error};
use avahi_sys::{
    avahi_simple_poll_free, avahi_simple_poll_iterate, avahi_simple_poll_loop,
    avahi_simple_poll_new, avahi_simple_poll_quit, AvahiSimplePoll,
};
use std::{convert::TryInto, sync::RwLock, time::Duration};

/// Wraps the `AvahiSimplePoll` type from the raw Avahi bindings.
///
/// This struct allocates a new `*mut AvahiSimplePoll` when `ManagedAvahiClient::new()` is invoked
/// and calls the Avahi function responsible for freeing the poll on `trait Drop`.
#[derive(Debug)]
pub struct ManagedAvahiSimplePoll {
    native: *mut AvahiSimplePoll,
    // Using RwLock only to avoid introducing breaking changes in `iterate` (which would otherwise
    // require `&mut self`).
    finished: RwLock<bool>,
}

impl ManagedAvahiSimplePoll {
    /// Initializes the underlying `*mut AvahiSimplePoll` and verifies it was created; returning
    /// `Err(String)` if unsuccessful
    ///
    /// # Safety
    /// This function is unsafe because of the raw pointer dereference.
    pub unsafe fn new() -> Result<Self> {
        let poll = avahi_simple_poll_new();
        if poll.is_null() {
            Err("could not initialize AvahiSimplePoll".into())
        } else {
            Ok(Self {
                native: poll,
                finished: RwLock::new(false),
            })
        }
    }

    /// Delegate function for [`avahi_simple_poll_loop()`].
    ///
    /// [`avahi_simple_poll_loop()`]: https://avahi.org/doxygen/html/simple-watch_8h.html#a14b4cb29832e8c3de609d4c4e5611985
    ///
    /// # Safety
    /// This function is unsafe because of the call to `avahi_simple_poll_loop()`.
    pub unsafe fn start_loop(&self) -> Result<()> {
        avahi_util::sys_exec(
            || avahi_simple_poll_loop(self.native),
            "could not start AvahiSimplePoll",
        )
    }

    /// Delegate function for [`avahi_simple_poll_iterate()`].
    ///
    /// [`avahi_simple_poll_iterate()`]: https://avahi.org/doxygen/html/simple-watch_8h.html#ad5b7c9d3b7a6584d609241ee6f472a2e
    ///
    /// # Safety
    /// This function is unsafe because of the call to `avahi_simple_poll_iterate()`.
    pub unsafe fn iterate(&self, timeout: Duration) -> Result<()> {
        let sleep_time: i32 = timeout
            .as_millis() // `avahi_simple_poll_iterate()` expects `sleep_time` in msecs.
            .try_into() // `avahi_simple_poll_iterate()` expects `sleep_time` as an i32.
            .unwrap_or(i32::MAX); // if converting to an i32 overflows, just use the largest number we can.

        // Check to avoid assertion inside `avahi_simple_poll_iterate` when it's compiled in debug
        // mode.
        if self.has_finished() {
            return Err(Error::from(
                "avahi_simple_poll_iterate(..) already finished",
            ));
        }

        // Returns -1 on error, 0 on success and 1 if a quit request has been scheduled
        match avahi_simple_poll_iterate(self.native, sleep_time) {
            0 => Ok(()),
            1 => {
                self.mark_finished();
                Err(Error::from("avahi_simple_poll_iterate(..) disconnected"))
            }
            -1 => {
                self.mark_finished();
                Err(Error::from(
                    "avahi_simple_poll_iterate(..) threw an error result",
                ))
            }
            _ => {
                self.mark_finished();
                Err(Error::from(
                    "avahi_simple_poll_iterate(..) returned an unknown result",
                ))
            }
        }
    }

    pub(super) fn inner(&self) -> *mut AvahiSimplePoll {
        self.native
    }

    pub(crate) unsafe fn quit(&self) {
        avahi_simple_poll_quit(self.native);
    }

    fn has_finished(&self) -> bool {
        *self.finished.read().expect("could not acquire read lock")
    }

    fn mark_finished(&self) {
        *self.finished.write().expect("could not acquire write lock") = true;
    }
}

impl Drop for ManagedAvahiSimplePoll {
    fn drop(&mut self) {
        unsafe { avahi_simple_poll_free(self.native) };
    }
}

unsafe impl Send for ManagedAvahiSimplePoll {}
unsafe impl Sync for ManagedAvahiSimplePoll {}

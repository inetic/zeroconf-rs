//! Rust friendly `AvahiClient` wrappers/helpers

use std::sync::Arc;

use super::avahi_util;
use super::poll::ManagedAvahiSimplePoll;
use crate::ffi::c_str;
use crate::{Error, Result};
use avahi_sys::{
    avahi_client_free, avahi_client_get_host_name, avahi_client_new, avahi_simple_poll_get,
    AvahiClient, AvahiClientCallback, AvahiClientFlags,
};
use libc::{c_int, c_void};

/// Wraps the `AvahiClient` type from the raw Avahi bindings.
///
/// This struct allocates a new `*mut AvahiClient` when `ManagedAvahiClient::new()` is invoked and
/// calls the Avahi function responsible for freeing the client on `trait Drop`.
#[derive(Debug)]
pub struct ManagedAvahiClient {
    pub(crate) inner: *mut AvahiClient,
    _poll: Arc<ManagedAvahiSimplePoll>,
}

impl ManagedAvahiClient {
    /// Initializes the underlying `*mut AvahiClient` and verifies it was created; returning
    /// `Err(String)` if unsuccessful.
    ///
    /// # Safety
    /// This function is unsafe because of the raw pointer dereference.
    pub unsafe fn new(
        ManagedAvahiClientParams {
            poll,
            flags,
            callback,
            userdata,
        }: ManagedAvahiClientParams,
    ) -> Result<Self> {
        let mut err: c_int = 0;

        let inner = avahi_client_new(
            avahi_simple_poll_get(poll.inner()),
            flags,
            callback,
            userdata,
            &mut err,
        );

        if inner.is_null() {
            return Err(Error::MdnsSystemError {
                code: err,
                message: avahi_util::get_error(err).into(),
            });
        }

        Ok(Self { inner, _poll: poll })
    }

    /// Delegate function for [`avahi_client_get_host_name()`].
    ///
    /// [`avahi_client_get_host_name()`]: https://avahi.org/doxygen/html/client_8h.html#a89378618c3c592a255551c308ba300bf
    ///
    /// # Safety
    /// This function is unsafe because of the raw pointer dereference.
    pub unsafe fn host_name<'a>(&self) -> Result<&'a str> {
        get_host_name(self.inner)
    }
}

impl Drop for ManagedAvahiClient {
    fn drop(&mut self) {
        unsafe { avahi_client_free(self.inner) };
    }
}

unsafe impl Send for ManagedAvahiClient {}
unsafe impl Sync for ManagedAvahiClient {}

/// Holds parameters for initializing a new `ManagedAvahiClient` with `ManagedAvahiClient::new()`.
///
/// See [`avahi_client_new()`] for more information about these parameters.
///
/// [`avahi_client_new()`]: https://avahi.org/doxygen/html/client_8h.html#a07b2a33a3e7cbb18a0eb9d00eade6ae6
#[derive(Builder, BuilderDelegate)]
pub struct ManagedAvahiClientParams {
    poll: Arc<ManagedAvahiSimplePoll>,
    flags: AvahiClientFlags,
    callback: AvahiClientCallback,
    userdata: *mut c_void,
}

pub(super) unsafe fn get_host_name<'a>(client: *mut AvahiClient) -> Result<&'a str> {
    assert_not_null!(client);
    let host_name = avahi_client_get_host_name(client);

    if !host_name.is_null() {
        Ok(c_str::raw_to_str(host_name))
    } else {
        Err(avahi_util::get_last_error(client))
    }
}

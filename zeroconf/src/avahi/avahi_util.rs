//! Utilities related to Avahi

use crate::{ffi::c_str, Error};
use avahi_sys::{
    avahi_address_snprint, avahi_alternative_service_name, avahi_strerror, AvahiAddress,
    AvahiClient,
};
use libc::c_char;
use std::ffi::CStr;

use crate::{NetworkInterface, Result, ServiceType};

/// Converts the specified `*const AvahiAddress` to a `String`.
///
/// The new `String` is constructed through allocating a new `CString`, passing it to
/// `avahi_address_snprint` and then converting it to a Rust-type `String`.
///
/// # Safety
/// This function is unsafe because of internal Avahi calls and raw pointer dereference.
pub unsafe fn avahi_address_to_string(addr: *const AvahiAddress) -> String {
    assert_not_null!(addr);

    let addr_str = c_string!(alloc(avahi_sys::AVAHI_ADDRESS_STR_MAX as usize));

    avahi_address_snprint(
        addr_str.as_ptr() as *mut c_char,
        avahi_sys::AVAHI_ADDRESS_STR_MAX as usize,
        addr,
    );

    String::from(c_str::to_str(&addr_str))
        .trim_matches(char::from(0))
        .to_string()
}

/// Returns the `&str` message associated with the specified error code.
///
/// # Safety
/// This function is unsafe because of internal Avahi calls.
pub unsafe fn get_error<'a>(code: i32) -> &'a str {
    CStr::from_ptr(avahi_strerror(code))
        .to_str()
        .expect("could not fetch Avahi error string")
}

/// Returns the last error message associated with the specified `*mut AvahiClient`.
///
/// # Safety
/// This function is unsafe because of internal Avahi calls.
pub unsafe fn get_last_error(client: *mut AvahiClient) -> Error {
    let code = avahi_sys::avahi_client_errno(client);
    let message = get_error(code);

    Error::MdnsSystemError {
        code,
        message: message.into(),
    }
}

/// Converts the specified [`NetworkInterface`] to the Avahi expected value.
///
/// [`NetworkInterface`]: ../../enum.NetworkInterface.html
pub fn interface_index(interface: NetworkInterface) -> i32 {
    match interface {
        NetworkInterface::Unspec => avahi_sys::AVAHI_IF_UNSPEC,
        NetworkInterface::AtIndex(i) => i as i32,
    }
}

/// Converts the specified Avahi interface index to a [`NetworkInterface`].
pub fn interface_from_index(index: i32) -> NetworkInterface {
    match index {
        avahi_sys::AVAHI_IF_UNSPEC => NetworkInterface::Unspec,
        _ => NetworkInterface::AtIndex(index as u32),
    }
}

/// Executes the specified closure and returns a formatted `Result`
///
/// # Safety
/// This function is unsafe because of the call to `get_error`.
pub unsafe fn sys_exec<F: FnOnce() -> i32>(func: F, message: &str) -> Result<()> {
    let err = func();

    if err < 0 {
        Err(Error::MdnsSystemError {
            code: err,
            message: format!("{}: (code: {}, message:{:?})", message, err, get_error(err)).into(),
        })
    } else {
        Ok(())
    }
}

/// Formats the specified `ServiceType` as a `String` for use with Avahi
pub fn format_service_type(service_type: &ServiceType) -> String {
    format!("_{}._{}", service_type.name(), service_type.protocol())
}

/// Formats the specified `ServiceType` as a `String` for browsing Avahi services
pub fn format_browser_type(service_type: &ServiceType) -> String {
    let kind = format_service_type(service_type);
    let sub_types = service_type.sub_types();

    if sub_types.is_empty() {
        return kind;
    }

    if sub_types.len() > 1 {
        warn!("browsing by multiple sub-types is not supported on Avahi devices, using first sub-type only");
    }

    format_sub_type(&sub_types[0], &kind)
}

/// Formats the specified `sub_type` string as a `String` for use with Avahi
pub fn format_sub_type(sub_type: &str, kind: &str) -> String {
    format!(
        "{}{}._sub.{}",
        if sub_type.starts_with('_') { "" } else { "_" },
        sub_type,
        kind
    )
}

/// Returns an alternative service name for the specified `CStr`
///
/// # Safety
/// This function is unsafe because of the call to `avahi_alternative_service_name`.
pub unsafe fn alternative_service_name(name: &CStr) -> &CStr {
    CStr::from_ptr(avahi_alternative_service_name(name.as_ptr()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use avahi_sys::{
        AvahiAddress__bindgen_ty_1, AvahiIPv4Address, AvahiIPv6Address, AVAHI_PROTO_INET,
        AVAHI_PROTO_INET6,
    };

    #[test]
    fn sys_exec_returns_ok_for_success() {
        assert!(unsafe { sys_exec(|| 0, "test") }.is_ok());
    }

    #[test]
    fn sys_exec_returns_error_for_failure() {
        assert_eq!(
            unsafe { sys_exec(|| avahi_sys::AVAHI_ERR_FAILURE, "uh oh spaghetti-o") },
            Err(Error::MdnsSystemError {
                code: avahi_sys::AVAHI_ERR_FAILURE,
                message: "uh oh spaghetti-o: (code: -1, message:\"Operation failed\")".into(),
            })
        );
    }

    #[test]
    fn interface_index_returns_unspec_for_unspec() {
        assert_eq!(
            interface_index(NetworkInterface::Unspec),
            avahi_sys::AVAHI_IF_UNSPEC
        );
    }

    #[test]
    fn interface_index_returns_index_for_index() {
        assert_eq!(interface_index(NetworkInterface::AtIndex(1)), 1);
    }

    #[test]
    fn interface_from_index_returns_unspec_for_avahi_unspec() {
        assert_eq!(
            interface_from_index(avahi_sys::AVAHI_IF_UNSPEC),
            NetworkInterface::Unspec
        );
    }

    #[test]
    fn interface_from_index_returns_index_for_avahi_index() {
        assert_eq!(interface_from_index(1), NetworkInterface::AtIndex(1));
    }

    #[test]
    fn format_service_type_returns_valid_string() {
        assert_eq!(
            format_service_type(&ServiceType::new("http", "tcp").unwrap()),
            "_http._tcp"
        );
    }

    #[test]
    fn format_browser_type_returns_valid_string() {
        assert_eq!(
            format_browser_type(&ServiceType::new("http", "tcp").unwrap()),
            "_http._tcp"
        );
    }

    #[test]
    fn format_browser_type_returns_string_with_sub_types() {
        assert_eq!(
            format_browser_type(
                &ServiceType::with_sub_types("http", "tcp", vec!["printer1", "printer2"]).unwrap()
            ),
            "_printer1._sub._http._tcp"
        );
    }

    #[test]
    fn format_sub_type_returns_valid_string() {
        assert_eq!(format_sub_type("foo", "_http._tcp"), "_foo._sub._http._tcp");
    }

    #[test]
    fn format_sub_type_strips_leading_underscore() {
        assert_eq!(
            format_sub_type("_foo", "_http._tcp"),
            "_foo._sub._http._tcp"
        );
    }

    #[test]
    fn get_error_returns_valid_error_string() {
        assert_eq!(
            unsafe { get_error(avahi_sys::AVAHI_ERR_FAILURE) },
            "Operation failed"
        );
    }

    #[test]
    fn address_to_string_returns_correct_ipv4_string() {
        let ipv4_addr = AvahiAddress {
            proto: AVAHI_PROTO_INET,
            data: AvahiAddress__bindgen_ty_1 {
                ipv4: AvahiIPv4Address {
                    address: 0x6464a8c0, // 192.168.100.100
                },
            },
        };

        unsafe {
            assert_eq!(avahi_address_to_string(&ipv4_addr), "192.168.100.100");
        }
    }

    #[test]
    fn address_to_string_returns_correct_ipv6_string() {
        let ipv6_addr = AvahiAddress {
            proto: AVAHI_PROTO_INET6,
            data: AvahiAddress__bindgen_ty_1 {
                ipv6: AvahiIPv6Address {
                    address: [
                        0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x56, 0x78,
                        0x9a, 0xbc, 0xde, 0xf0,
                    ],
                },
            },
        };

        unsafe {
            assert_eq!(
                avahi_address_to_string(&ipv6_addr),
                "fe80::1234:5678:9abc:def0"
            );
        }
    }
}

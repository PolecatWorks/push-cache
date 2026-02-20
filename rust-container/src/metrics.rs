use std::ffi::{CString, c_char, c_void};

use prometheus::{Encoder, Registry};
use tracing::info;

use crate::MyState;

#[unsafe(no_mangle)]
pub extern "C" fn prometheus_response(ptr: *const c_void) -> *const c_char {
    info!("Gathering Prometheus metrics in bill");

    let registry = unsafe { &*(ptr as *const Registry) };

    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();

    let metric_families = registry.gather();

    encoder.encode(&metric_families, &mut buffer).unwrap();

    let prometheus = String::from_utf8(buffer).unwrap();

    let c_str_prometheus = std::ffi::CString::new(prometheus).unwrap();

    c_str_prometheus.into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn prometheus_response_mystate(ptr: *const c_void) -> *const c_char {
    let state = unsafe { &*(ptr as *const MyState) };

    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();

    let metric_families = state.registry.gather();

    encoder.encode(&metric_families, &mut buffer).unwrap();

    let axum_string = state.prometheus_handle.render();

    let axum_bytes = axum_string.as_bytes();
    buffer.extend_from_slice(axum_bytes);

    let prometheus = String::from_utf8(buffer).unwrap();

    let c_str_prometheus = std::ffi::CString::new(prometheus).unwrap();

    c_str_prometheus.into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn prometheus_response_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    };
}

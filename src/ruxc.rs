//! RUXC
//!
//! Rust Utilities eXported to C
//!
//! Useful functions written in Rust made available to C
//!   * simple HTTP/S client funtions for GET or POST requests

extern crate libc;

use rustls;
use std;
use std::collections::HashMap;
use std::io::Read;
use ureq;
use url;

thread_local!(static HTTPAGENT: std::cell::RefCell<ureq::Agent> = std::cell::RefCell::new(ureq::Agent::new()));

thread_local! {
    static HTTPAGENTMAP: std::cell::RefCell< HashMap<String, ureq::Agent> > = HashMap::new().into();
}

static mut HTTPAGENTREADY: u32 = 0;

#[derive(PartialEq)]
enum HTTPMethodType {
    MethodGET,
    MethodPOST,
    MethodDELETE,
    MethodCUSTOM,
}

#[repr(C)]
pub struct RuxcHTTPRequest {
    pub method: *const libc::c_char,
    pub url: *const libc::c_char,
    pub url_len: libc::c_int,
    pub headers: *const libc::c_char,
    pub headers_len: libc::c_int,
    pub data: *const libc::c_char,
    pub data_len: libc::c_int,
    pub timeout: libc::c_int,
    pub timeout_connect: libc::c_int,
    pub timeout_read: libc::c_int,
    pub timeout_write: libc::c_int,
    pub tlsmode: libc::c_int,
    pub flags: libc::c_int,
    pub debug: libc::c_int,
    pub reuse: libc::c_int,
    pub retry: libc::c_int,
    pub logtype: libc::c_int,
}

#[repr(C)]
pub struct RuxcHTTPResponse {
    pub retcode: libc::c_int,
    pub rescode: libc::c_int,
    pub resdata: *mut libc::c_char,
    pub resdata_len: libc::c_int,
}

const RUXC_HTTP_RET_ERROR: libc::c_int = -1;
const RUXC_HTTP_RET_INVALID_ARGUMENT: libc::c_int = -20;
const RUXC_HTTP_RET_INVALID_INPUT: libc::c_int = -21;
const RUXC_HTTP_RET_PANIC: libc::c_int = -99;

unsafe fn ruxc_http_response_init(v_http_response: *mut RuxcHTTPResponse) {
    (*v_http_response).retcode = RUXC_HTTP_RET_ERROR;
    (*v_http_response).rescode = 0;
    (*v_http_response).resdata = std::ptr::null_mut();
    (*v_http_response).resdata_len = 0;
}

#[no_mangle]
pub extern "C" fn ruxc_http_response_release(v_http_response: *mut RuxcHTTPResponse) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        if v_http_response.is_null() {
            return;
        }

        let response = &mut *v_http_response;
        if response.resdata.is_null() {
            response.resdata_len = 0;
            return;
        }

        if let Some(allocation_len) = usize::try_from(response.resdata_len)
            .ok()
            .and_then(|len| len.checked_add(1))
        {
            let allocation =
                std::ptr::slice_from_raw_parts_mut(response.resdata.cast::<u8>(), allocation_len);
            drop(Box::from_raw(allocation));
        }
        response.resdata = std::ptr::null_mut();
        response.resdata_len = 0;
    }));
}

#[derive(Debug)]
struct StringError(String);

impl std::error::Error for StringError {}

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for StringError {
    fn from(source: String) -> Self {
        Self(source)
    }
}

#[derive(Debug)]
struct Error {
    source: Box<dyn std::error::Error>,
    retcode: libc::c_int,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl From<StringError> for Error {
    fn from(source: StringError) -> Self {
        Error {
            source: source.into(),
            retcode: RUXC_HTTP_RET_INVALID_INPUT,
        }
    }
}

impl From<ureq::Error> for Error {
    fn from(source: ureq::Error) -> Self {
        Error {
            source: source.into(),
            retcode: RUXC_HTTP_RET_ERROR,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error {
            source: source.into(),
            retcode: RUXC_HTTP_RET_ERROR,
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(source: url::ParseError) -> Self {
        Error {
            source: source.into(),
            retcode: RUXC_HTTP_RET_INVALID_INPUT,
        }
    }
}

unsafe fn ruxc_buffer_from_raw_parts<'a>(
    ptr: *const libc::c_char,
    len: libc::c_int,
    field: &str,
) -> Result<&'a [u8], Error> {
    if len < 0 {
        return Err(StringError::from(format!("{} length must not be negative", field)).into());
    }
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(StringError::from(format!(
            "{} pointer is null while its length is positive",
            field
        ))
        .into());
    }

    Ok(std::slice::from_raw_parts(ptr.cast::<u8>(), len as usize))
}

unsafe fn ruxc_utf8_buffer_from_raw_parts<'a>(
    ptr: *const libc::c_char,
    len: libc::c_int,
    field: &str,
) -> Result<&'a str, Error> {
    let bytes = ruxc_buffer_from_raw_parts(ptr, len, field)?;
    std::str::from_utf8(bytes)
        .map_err(|err| StringError::from(format!("{} is not valid UTF-8: {}", field, err)).into())
}

struct TLSAcceptAllCerts {}

impl rustls::client::ServerCertVerifier for TLSAcceptAllCerts {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

// logtype: 0 - stdout; 1 - syslog
// debug: threshold to filter based on level value
// level: 0 - no logs; 1 - errors; 2 - infos; 3 - debugs
fn ruxc_print_log(logtype: i32, debug: i32, level: i32, message: String) {
    if level > debug {
        return;
    }
    if logtype == 0 {
        if level == 1 {
            println!("* ruxc [error]:: {}", message);
        } else if level == 2 {
            println!("* ruxc [info]:: {}", message);
        } else if level == 3 {
            println!("* ruxc [debug]:: {}", message);
        }
    } else if logtype == 1 {
        let c_message = match std::ffi::CString::new(message.replace('\0', "\\0")) {
            Ok(message) => message,
            Err(_) => return,
        };
        let c_fmt = unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(b"%s\n\0") };
        unsafe {
            if level == 1 {
                libc::syslog(libc::LOG_ERR, c_fmt.as_ptr(), c_message.as_ptr());
            } else if level == 2 {
                libc::syslog(libc::LOG_INFO, c_fmt.as_ptr(), c_message.as_ptr());
            } else if level == 3 {
                libc::syslog(libc::LOG_DEBUG, c_fmt.as_ptr(), c_message.as_ptr());
            }
        }
    }
}

fn ruxc_timeout_from_millis(
    value: libc::c_int,
    field: &str,
) -> Result<Option<std::time::Duration>, Error> {
    if value < 0 {
        return Err(StringError::from(format!("{} must not be negative", field)).into());
    }
    if value == 0 {
        return Ok(None);
    }

    Ok(Some(std::time::Duration::from_millis(value as u64)))
}

fn ruxc_http_agent_builder(
    v_http_request: *const RuxcHTTPRequest,
) -> Result<ureq::AgentBuilder, Error> {
    let v_tlsmode = unsafe { (*v_http_request).tlsmode as i32 };
    let v_timeout_connect =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout_connect, "connect timeout")? };
    let v_timeout_read =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout_read, "read timeout")? };
    let v_timeout_write =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout_write, "write timeout")? };
    let v_timeout =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout, "overall timeout")? };

    let mut builder = ureq::builder();

    if let Some(timeout) = v_timeout_connect {
        builder = builder.timeout_connect(timeout)
    }
    if let Some(timeout) = v_timeout_read {
        builder = builder.timeout_read(timeout)
    }
    if let Some(timeout) = v_timeout_write {
        builder = builder.timeout_write(timeout)
    }
    if let Some(timeout) = v_timeout {
        builder = builder.timeout(timeout);
    }

    if v_tlsmode == 0 {
        let mut client_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth();
        client_config
            .dangerous()
            .set_certificate_verifier(std::sync::Arc::new(TLSAcceptAllCerts {}));
        builder = builder.tls_config(std::sync::Arc::new(client_config));
    }

    Ok(builder)
}

fn ruxc_http_response_storing(status: u16, final_attempt: bool) -> bool {
    final_attempt || (200..=299).contains(&status)
}

fn ruxc_http_response_store_body(
    v_http_response: *mut RuxcHTTPResponse,
    mut body: Vec<u8>,
) -> Result<(), Error> {
    let body_len = libc::c_int::try_from(body.len()).map_err(|_| Error {
        source: StringError::from("HTTP response body is too large".to_owned()).into(),
        retcode: RUXC_HTTP_RET_ERROR,
    })?;

    unsafe {
        (*v_http_response).resdata = std::ptr::null_mut();
        (*v_http_response).resdata_len = body_len;
        if body_len > 0 {
            body.push(0);
            let body = body.into_boxed_slice();
            (*v_http_response).resdata = Box::into_raw(body).cast::<u8>().cast::<libc::c_char>();
        }
    }

    Ok(())
}

fn ruxc_http_request_perform(
    agent: &ureq::Agent,
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: &HTTPMethodType,
    final_attempt: bool,
) -> Result<(), Error> {
    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    let r_met_str = unsafe {
        if !(*v_http_request).method.is_null() {
            std::ffi::CStr::from_ptr((*v_http_request).method)
                .to_str()
                .map_err(|err| {
                    StringError::from(format!("HTTP method is not valid UTF-8: {}", err))
                })?
        } else {
            "GET"
        }
    };

    let r_url_str = unsafe {
        ruxc_utf8_buffer_from_raw_parts((*v_http_request).url, (*v_http_request).url_len, "URL")?
    };

    let mut req: ureq::Request;

    match *v_method {
        HTTPMethodType::MethodPOST => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP POST - url: {}", r_url_str),
                );
            }
            req = agent.post(r_url_str);
        }
        HTTPMethodType::MethodDELETE => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP DELETE - url: {}", r_url_str),
                );
            }
            req = agent.delete(r_url_str);
        }
        HTTPMethodType::MethodCUSTOM => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP CUSTOM {} - url: {}", r_met_str, r_url_str),
                );
            }
            req = agent.request(r_met_str, r_url_str);
        }
        _ => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP GET - url: {}", r_url_str),
                );
            }
            req = agent.get(r_url_str);
        }
    }

    unsafe {
        let r_headers_str = ruxc_utf8_buffer_from_raw_parts(
            (*v_http_request).headers,
            (*v_http_request).headers_len,
            "headers",
        )?;
        if !r_headers_str.is_empty() {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    3,
                    format!("adding headers: [[{}]]", r_headers_str),
                );
            }
            for line in r_headers_str.lines() {
                if let Some(cpos) = line.find(':').filter(|cpos| *cpos > 0) {
                    req = req.set(&line[0..cpos], &line[(cpos + 1)..].trim());
                }
            }
        }
    };

    let res: ureq::Response;
    let exres: std::result::Result<ureq::Response, ureq::Error>;

    if *v_method == HTTPMethodType::MethodPOST || *v_method == HTTPMethodType::MethodCUSTOM {
        let r_body = unsafe {
            ruxc_buffer_from_raw_parts(
                (*v_http_request).data,
                (*v_http_request).data_len,
                "request body",
            )?
        };
        if debug != 0 {
            ruxc_print_log(
                logtype,
                debug,
                3,
                format!("post body: [[{}]]", String::from_utf8_lossy(r_body)),
            );
        }
        exres = req.send_bytes(r_body);
    } else {
        if debug != 0 {
            ruxc_print_log(logtype, debug, 3, format!("get request"));
        }
        exres = req.call();
    }
    match exres {
        Ok(response) => {
            res = response;
        }
        Err(ureq::Error::Status(_, response)) => {
            res = response;
        }
        Err(err) => {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 1, format!("* ruxc:: error: {:?}", err));
            }
            return Ok(());
        }
    }

    if debug != 0 {
        ruxc_print_log(
            logtype,
            debug,
            3,
            format!(
                "* ruxc:: {} {} {}",
                res.http_version(),
                res.status(),
                res.status_text()
            ),
        );
    }

    unsafe {
        (*v_http_response).rescode = res.status() as i32;
    };

    if ruxc_http_response_storing(res.status(), final_attempt) {
        // Store successful responses immediately and the last response after
        // all retry attempts have been exhausted.
        let mut body = Vec::new();
        res.into_reader().read_to_end(&mut body)?;

        if debug != 0 {
            ruxc_print_log(
                logtype,
                debug,
                3,
                format!(
                    "* ruxc:: HTTP response body: {}",
                    String::from_utf8_lossy(&body)
                ),
            );
        }

        ruxc_http_response_store_body(v_http_response, body)?;
        unsafe {
            (*v_http_response).retcode = 0;
        }
    }

    return Ok(());
}

// Perform HTTP/S request with a new agent every time
fn ruxc_http_request_perform_once(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: HTTPMethodType,
) -> Result<(), Error> {
    unsafe {
        (*v_http_response).retcode = RUXC_HTTP_RET_ERROR;
        if (*v_http_request).url.is_null() {
            (*v_http_response).retcode = RUXC_HTTP_RET_INVALID_ARGUMENT;
            return Ok(());
        }
    };

    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    if debug != 0 {
        ruxc_print_log(
            logtype,
            debug,
            3,
            format!("initializing http agent - noreuse"),
        );
    }

    let builder = ruxc_http_agent_builder(v_http_request)?;

    let agent = builder.build();

    let mut retry = unsafe { (*v_http_request).retry as i32 };

    loop {
        ruxc_http_request_perform(
            &agent,
            v_http_request,
            v_http_response,
            &v_method,
            retry <= 0,
        )?;
        if retry <= 0 {
            break;
        }
        unsafe {
            if (*v_http_response).rescode >= 200 && (*v_http_response).rescode <= 299 {
                break;
            }
        }
        retry -= 1;
    }
    return Ok(());
}

// Perform HTTP/S request reusing one global agent every time
fn ruxc_http_request_perform_reuse(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: HTTPMethodType,
) -> Result<(), Error> {
    unsafe {
        (*v_http_response).retcode = RUXC_HTTP_RET_ERROR;
        if (*v_http_request).url.is_null() {
            (*v_http_response).retcode = RUXC_HTTP_RET_INVALID_ARGUMENT;
            return Ok(());
        }
    };

    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    let haready = unsafe { HTTPAGENTREADY as u32 };

    if haready == 0 {
        if debug != 0 {
            ruxc_print_log(
                logtype,
                debug,
                3,
                format!("initializing http agent - reuse on"),
            );
        }

        let builder = ruxc_http_agent_builder(v_http_request)?;

        HTTPAGENT.with(|agent| {
            *agent.borrow_mut() = builder.build();
        });
        if debug != 0 {
            ruxc_print_log(logtype, debug, 3, format!("saving ready state - reuse on"));
        }
        unsafe {
            HTTPAGENTREADY = 1;
        };
    }

    let mut retry = unsafe { (*v_http_request).retry as i32 };

    HTTPAGENT.with(|agent| -> Result<(), Error> {
        loop {
            ruxc_http_request_perform(
                &(*agent.borrow()),
                v_http_request,
                v_http_response,
                &v_method,
                retry <= 0,
            )?;
            if retry <= 0 {
                break;
            }
            unsafe {
                if (*v_http_response).rescode >= 200 && (*v_http_response).rescode <= 299 {
                    break;
                }
            }
            retry -= 1;
        }
        Ok(())
    })?;

    return Ok(());
}

// Perform HTTP/S request reusing agents kept in hashmap by base URL
fn ruxc_http_request_perform_hashmap(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: HTTPMethodType,
) -> Result<(), Error> {
    unsafe {
        (*v_http_response).retcode = RUXC_HTTP_RET_ERROR;
        if (*v_http_request).url.is_null() {
            (*v_http_response).retcode = RUXC_HTTP_RET_INVALID_ARGUMENT;
            return Ok(());
        }
    };

    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    let r_url_str = unsafe {
        ruxc_utf8_buffer_from_raw_parts((*v_http_request).url, (*v_http_request).url_len, "URL")?
            .to_owned()
    };

    let url = url::Url::parse(&r_url_str)?;

    let htkey = format!(
        "{}://{}:{}",
        url.scheme(),
        url.host_str().unwrap_or("127.0.0.1"),
        url.port_or_known_default().unwrap_or(80)
    );

    if debug != 0 {
        ruxc_print_log(logtype, debug, 3, format!("htable key [{}]", htkey));
    }

    HTTPAGENTMAP.with(|item| -> Result<(), Error> {
        let mut ht = item.borrow_mut();
        if !ht.contains_key(&htkey) {
            let htnewkey = String::clone(&htkey);
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    3,
                    format!("initializing http agent for [{}]", htnewkey),
                );
            }
            let builder = ruxc_http_agent_builder(v_http_request)?;
            ht.insert(htnewkey, builder.build());
        }
        if let Some(agent) = ht.get(&htkey) {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    3,
                    format!("agent retrieved for [{}]", htkey),
                );
            }
            let mut retry = unsafe { (*v_http_request).retry as i32 };
            loop {
                ruxc_http_request_perform(
                    &agent,
                    v_http_request,
                    v_http_response,
                    &v_method,
                    retry <= 0,
                )?;
                if retry <= 0 {
                    break;
                }
                unsafe {
                    if (*v_http_response).rescode >= 200 && (*v_http_response).rescode <= 299 {
                        break;
                    }
                }
                retry -= 1;
            }
        }
        Ok(())
    })?;

    return Ok(());
}

fn ruxc_http_request_dispatch(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: HTTPMethodType,
) -> Result<(), Error> {
    let reuse = unsafe { (*v_http_request).reuse as i32 };
    match reuse {
        1 => ruxc_http_request_perform_reuse(v_http_request, v_http_response, v_method),
        2 => ruxc_http_request_perform_hashmap(v_http_request, v_http_response, v_method),
        _ => ruxc_http_request_perform_once(v_http_request, v_http_response, v_method),
    }
}

fn ruxc_http_request_ffi(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: HTTPMethodType,
) -> libc::c_int {
    if v_http_response.is_null() {
        return RUXC_HTTP_RET_INVALID_ARGUMENT;
    }
    unsafe {
        ruxc_http_response_init(v_http_response);
    }
    if v_http_request.is_null() {
        unsafe {
            (*v_http_response).retcode = RUXC_HTTP_RET_INVALID_ARGUMENT;
        }
        return RUXC_HTTP_RET_INVALID_ARGUMENT;
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ruxc_http_request_dispatch(v_http_request, v_http_response, v_method)
    }));

    let retcode = match result {
        Ok(Ok(())) => unsafe { (*v_http_response).retcode },
        Ok(Err(err)) => err.retcode,
        Err(_) => RUXC_HTTP_RET_PANIC,
    };
    unsafe {
        (*v_http_response).retcode = retcode;
    }
    retcode
}

// Perform HTTP/S GET request
#[no_mangle]
pub extern "C" fn ruxc_http_get(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
) -> libc::c_int {
    ruxc_http_request_ffi(v_http_request, v_http_response, HTTPMethodType::MethodGET)
}

// Perform HTTP/S POST request
#[no_mangle]
pub extern "C" fn ruxc_http_post(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
) -> libc::c_int {
    ruxc_http_request_ffi(v_http_request, v_http_response, HTTPMethodType::MethodPOST)
}

// Perform HTTP/S DELETE request
#[no_mangle]
pub extern "C" fn ruxc_http_delete(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
) -> libc::c_int {
    ruxc_http_request_ffi(
        v_http_request,
        v_http_response,
        HTTPMethodType::MethodDELETE,
    )
}

// Perform HTTP/S CUSTOM request
#[no_mangle]
pub extern "C" fn ruxc_http_request(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
) -> libc::c_int {
    ruxc_http_request_ffi(
        v_http_request,
        v_http_response,
        HTTPMethodType::MethodCUSTOM,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ruxc_buffer_from_raw_parts, ruxc_http_request, ruxc_http_response_release,
        ruxc_http_response_store_body, ruxc_http_response_storing, ruxc_timeout_from_millis,
        ruxc_utf8_buffer_from_raw_parts, RuxcHTTPRequest, RuxcHTTPResponse,
        RUXC_HTTP_RET_INVALID_ARGUMENT, RUXC_HTTP_RET_INVALID_INPUT, RUXC_HTTP_RET_PANIC,
    };

    #[test]
    fn stores_only_successful_or_final_retry_responses() {
        assert!(ruxc_http_response_storing(200, false));
        assert!(!ruxc_http_response_storing(503, false));
        assert!(ruxc_http_response_storing(503, true));
    }

    #[test]
    fn reads_only_the_declared_buffer_length() {
        let data = b"abcXYZ";
        let value = unsafe {
            ruxc_utf8_buffer_from_raw_parts(data.as_ptr().cast(), 3, "test buffer").unwrap()
        };

        assert_eq!(value, "abc");
    }

    #[test]
    fn preserves_embedded_nul_bytes() {
        let data = b"a\0b";
        let value = unsafe {
            ruxc_buffer_from_raw_parts(data.as_ptr().cast(), data.len() as i32, "test buffer")
                .unwrap()
        };

        assert_eq!(value, data);
    }

    #[test]
    fn rejects_invalid_buffer_metadata() {
        assert!(unsafe { ruxc_buffer_from_raw_parts(std::ptr::null(), 1, "test buffer") }.is_err());
        assert!(
            unsafe { ruxc_buffer_from_raw_parts(std::ptr::null(), -1, "test buffer") }.is_err()
        );
    }

    #[test]
    fn stores_and_releases_response_bodies_with_embedded_nuls() {
        let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
        let body = b"a\0b".to_vec();

        ruxc_http_response_store_body(&mut response, body.clone()).unwrap();

        assert_eq!(response.resdata_len, body.len() as i32);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(response.resdata.cast::<u8>(), body.len() + 1) },
            b"a\0b\0"
        );

        ruxc_http_response_release(&mut response);
        assert!(response.resdata.is_null());
        assert_eq!(response.resdata_len, 0);

        ruxc_http_response_release(&mut response);
        assert!(response.resdata.is_null());
        assert_eq!(response.resdata_len, 0);
    }

    #[test]
    fn response_length_counts_utf8_bytes() {
        let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
        let body = "Grüße".as_bytes().to_vec();

        ruxc_http_response_store_body(&mut response, body.clone()).unwrap();

        assert_eq!(response.resdata_len, body.len() as i32);
        assert_ne!(body.len(), "Grüße".chars().count());
        assert_eq!(
            unsafe { std::slice::from_raw_parts(response.resdata.cast::<u8>(), body.len()) },
            body
        );

        ruxc_http_response_release(&mut response);
    }

    #[test]
    fn exported_request_rejects_null_arguments() {
        let mut response = RuxcHTTPResponse {
            retcode: 123,
            rescode: 456,
            resdata: std::ptr::NonNull::<libc::c_char>::dangling().as_ptr(),
            resdata_len: 789,
        };

        assert_eq!(
            ruxc_http_request(std::ptr::null(), &mut response),
            RUXC_HTTP_RET_INVALID_ARGUMENT
        );
        assert_eq!(response.retcode, RUXC_HTTP_RET_INVALID_ARGUMENT);
        assert_eq!(response.rescode, 0);
        assert!(response.resdata.is_null());
        assert_eq!(response.resdata_len, 0);
        assert_eq!(
            ruxc_http_request(std::ptr::null(), std::ptr::null_mut()),
            RUXC_HTTP_RET_INVALID_ARGUMENT
        );
    }

    #[test]
    fn exported_request_rejects_non_utf8_methods_without_panicking() {
        let method = [0xff_u8, 0];
        let url = b"http://127.0.0.1/";
        let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
        let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
        request.method = method.as_ptr().cast();
        request.url = url.as_ptr().cast();
        request.url_len = url.len() as i32;

        assert_eq!(
            ruxc_http_request(&request, &mut response),
            RUXC_HTTP_RET_INVALID_INPUT
        );
        assert_eq!(response.retcode, RUXC_HTTP_RET_INVALID_INPUT);
    }

    #[test]
    fn validates_timeout_values_before_conversion() {
        assert!(ruxc_timeout_from_millis(-1, "test timeout").is_err());
        assert_eq!(ruxc_timeout_from_millis(0, "test timeout").unwrap(), None);
        assert_eq!(
            ruxc_timeout_from_millis(25, "test timeout").unwrap(),
            Some(std::time::Duration::from_millis(25))
        );

        let url = b"http://127.0.0.1/";
        let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
        let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
        request.url = url.as_ptr().cast();
        request.url_len = url.len() as i32;
        request.timeout = -1;

        assert_eq!(
            super::ruxc_http_get(&request, &mut response),
            RUXC_HTTP_RET_INVALID_INPUT
        );
        assert_eq!(response.retcode, RUXC_HTTP_RET_INVALID_INPUT);
    }

    #[test]
    fn exported_request_contains_internal_panics() {
        let url = b"http://127.0.0.1/";
        let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
        let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
        request.url = url.as_ptr().cast();
        request.url_len = url.len() as i32;
        request.reuse = 2;

        super::HTTPAGENTMAP.with(|agents| {
            let _borrow = agents.borrow_mut();
            assert_eq!(
                super::ruxc_http_get(&request, &mut response),
                RUXC_HTTP_RET_PANIC
            );
        });
        assert_eq!(response.retcode, RUXC_HTTP_RET_PANIC);
    }
}

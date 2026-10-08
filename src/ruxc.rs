//! RUXC
//!
//! Rust Utilities eXported to C
//!
//! Useful functions written in Rust made available to C
//!   * simple HTTP/S client funtions for GET or POST requests

extern crate libc;

use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! {
    static HTTPAGENT: std::cell::RefCell<Option<ureq::Agent>> = const { std::cell::RefCell::new(None) };
    static HTTPAGENTMAP: std::cell::RefCell< HashMap<String, ureq::Agent> > = HashMap::new().into();
}

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
const RUXC_HTTP_RET_RESPONSE_TOO_LARGE: libc::c_int = -22;
const RUXC_HTTP_RET_PANIC: libc::c_int = -99;
const RUXC_HTTP_DEFAULT_MAX_RESPONSE_SIZE: usize = 16 * 1024 * 1024;

static HTTP_MAX_RESPONSE_SIZE: AtomicUsize = AtomicUsize::new(RUXC_HTTP_DEFAULT_MAX_RESPONSE_SIZE);

#[no_mangle]
pub extern "C" fn ruxc_http_set_max_response_size(max_response_size: libc::size_t) -> libc::c_int {
    if max_response_size == 0 || max_response_size > libc::c_int::MAX as usize {
        return RUXC_HTTP_RET_INVALID_ARGUMENT;
    }

    HTTP_MAX_RESPONSE_SIZE.store(max_response_size, Ordering::SeqCst);
    0
}

#[no_mangle]
pub extern "C" fn ruxc_http_get_max_response_size() -> libc::size_t {
    HTTP_MAX_RESPONSE_SIZE.load(Ordering::SeqCst)
}

unsafe fn ruxc_http_response_init(v_http_response: *mut RuxcHTTPResponse) {
    (*v_http_response).retcode = RUXC_HTTP_RET_ERROR;
    (*v_http_response).rescode = 0;
    (*v_http_response).resdata = std::ptr::null_mut();
    (*v_http_response).resdata_len = 0;
}

#[no_mangle]
/// Release a response body allocated by ruxc.
///
/// # Safety
///
/// `v_http_response` must be null or point to a valid, writable `RuxcHTTPResponse`.
/// Any non-null `resdata` must have been returned by ruxc and not already released.
pub unsafe extern "C" fn ruxc_http_response_release(v_http_response: *mut RuxcHTTPResponse) {
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
        return Err(StringError::from(format!("{field} length must not be negative")).into());
    }
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(StringError::from(format!(
            "{field} pointer is null while its length is positive"
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
        .map_err(|err| StringError::from(format!("{field} is not valid UTF-8: {err}")).into())
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
            println!("* ruxc [error]:: {message}");
        } else if level == 2 {
            println!("* ruxc [info]:: {message}");
        } else if level == 3 {
            println!("* ruxc [debug]:: {message}");
        }
    } else if logtype == 1 {
        let c_message = match std::ffi::CString::new(message.replace('\0', "\\0")) {
            Ok(message) => message,
            Err(_) => return,
        };
        let c_fmt = c"%s\n";
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
        return Err(StringError::from(format!("{field} must not be negative")).into());
    }
    if value == 0 {
        return Ok(None);
    }

    Ok(Some(std::time::Duration::from_millis(value as u64)))
}

fn ruxc_http_agent_builder(v_http_request: *const RuxcHTTPRequest) -> Result<ureq::Agent, Error> {
    let v_tlsmode = unsafe { (*v_http_request).tlsmode };
    let v_timeout_connect =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout_connect, "connect timeout")? };
    let v_timeout_read =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout_read, "read timeout")? };
    let v_timeout_write =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout_write, "write timeout")? };
    let v_timeout =
        unsafe { ruxc_timeout_from_millis((*v_http_request).timeout, "overall timeout")? };

    let mut builder = ureq::Agent::config_builder()
        .allow_non_standard_methods(true)
        .http_status_as_error(false)
        .max_redirects(5)
        .proxy(None)
        .timeout_connect(v_timeout_connect)
        .timeout_recv_response(v_timeout_read)
        .timeout_recv_body(v_timeout_read)
        .timeout_send_request(v_timeout_write)
        .timeout_send_body(v_timeout_write)
        .timeout_global(v_timeout);

    if v_tlsmode == 0 {
        let tls_config = ureq::tls::TlsConfig::builder()
            .disable_verification(true)
            .build();
        builder = builder.tls_config(tls_config);
    }

    Ok(ureq::Agent::new_with_config(builder.build()))
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

fn ruxc_http_response_read_body<R: Read>(reader: R, max_size: usize) -> Result<Vec<u8>, Error> {
    let read_limit = max_size.checked_add(1).ok_or_else(|| Error {
        source: StringError::from("HTTP response size limit is too large".to_owned()).into(),
        retcode: RUXC_HTTP_RET_INVALID_ARGUMENT,
    })?;
    let mut body = Vec::new();
    reader.take(read_limit as u64).read_to_end(&mut body)?;

    if body.len() > max_size {
        return Err(Error {
            source: StringError::from(format!(
                "HTTP response body exceeds the configured limit of {max_size} bytes"
            ))
            .into(),
            retcode: RUXC_HTTP_RET_RESPONSE_TOO_LARGE,
        });
    }

    Ok(body)
}

fn ruxc_http_request_perform(
    agent: &ureq::Agent,
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: &HTTPMethodType,
    final_attempt: bool,
) -> Result<(), Error> {
    let debug = unsafe { (*v_http_request).debug };
    let logtype = unsafe { (*v_http_request).logtype };

    let r_met_str = unsafe {
        if !(*v_http_request).method.is_null() {
            std::ffi::CStr::from_ptr((*v_http_request).method)
                .to_str()
                .map_err(|err| {
                    StringError::from(format!("HTTP method is not valid UTF-8: {err}"))
                })?
        } else {
            "GET"
        }
    };

    let r_url_str = unsafe {
        ruxc_utf8_buffer_from_raw_parts((*v_http_request).url, (*v_http_request).url_len, "URL")?
    };

    let method = match *v_method {
        HTTPMethodType::MethodPOST => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP POST - url: {r_url_str}"),
                );
            }
            "POST"
        }
        HTTPMethodType::MethodDELETE => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP DELETE - url: {r_url_str}"),
                );
            }
            "DELETE"
        }
        HTTPMethodType::MethodCUSTOM => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP CUSTOM {r_met_str} - url: {r_url_str}"),
                );
            }
            r_met_str
        }
        _ => {
            if debug != 0 {
                ruxc_print_log(
                    logtype,
                    debug,
                    2,
                    format!("doing HTTP GET - url: {r_url_str}"),
                );
            }
            "GET"
        }
    };

    let mut req = ureq::http::Request::builder().method(method).uri(r_url_str);

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
                    format!("adding headers: [[{r_headers_str}]]"),
                );
            }
            for line in r_headers_str.lines() {
                if let Some(cpos) = line.find(':').filter(|cpos| *cpos > 0) {
                    let name = ureq::http::header::HeaderName::from_bytes(&line.as_bytes()[..cpos])
                        .map_err(|err| {
                            StringError::from(format!("invalid HTTP header name: {err}"))
                        })?;
                    let value =
                        ureq::http::header::HeaderValue::from_str(line[(cpos + 1)..].trim())
                            .map_err(|err| {
                                StringError::from(format!("invalid HTTP header value: {err}"))
                            })?;
                    let headers = req.headers_mut().ok_or_else(|| {
                        Error::from(StringError::from(
                            "failed to construct HTTP request headers".to_owned(),
                        ))
                    })?;
                    headers.insert(name, value);
                }
            }
        }
    };

    let exres =
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
            agent.run(req.body(r_body).map_err(|err| {
                Error::from(StringError::from(format!(
                    "failed to construct HTTP request: {err}"
                )))
            })?)
        } else {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 3, "get request".to_string());
            }
            agent.run(req.body(()).map_err(|err| {
                Error::from(StringError::from(format!(
                    "failed to construct HTTP request: {err}"
                )))
            })?)
        };
    let res = match exres {
        Ok(response) => response,
        Err(err) => {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 1, format!("* ruxc:: error: {err:?}"));
            }
            return Ok(());
        }
    };

    if debug != 0 {
        ruxc_print_log(
            logtype,
            debug,
            3,
            format!("* ruxc:: {:?} {}", res.version(), res.status()),
        );
    }

    let status = res.status().as_u16();
    unsafe {
        (*v_http_response).rescode = status as i32;
    };

    if ruxc_http_response_storing(status, final_attempt) {
        // Store successful responses immediately and the last response after
        // all retry attempts have been exhausted.
        let max_response_size = HTTP_MAX_RESPONSE_SIZE.load(Ordering::SeqCst);
        let body = ruxc_http_response_read_body(res.into_body().into_reader(), max_response_size)?;

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

    Ok(())
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

    let debug = unsafe { (*v_http_request).debug };
    let logtype = unsafe { (*v_http_request).logtype };

    if debug != 0 {
        ruxc_print_log(
            logtype,
            debug,
            3,
            "initializing http agent - noreuse".to_string(),
        );
    }

    let agent = ruxc_http_agent_builder(v_http_request)?;

    let mut retry = unsafe { (*v_http_request).retry };

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
    Ok(())
}

fn ruxc_http_agent_initialize(v_http_request: *const RuxcHTTPRequest) -> Result<bool, Error> {
    HTTPAGENT.with(|agent| {
        if agent.borrow().is_some() {
            return Ok(false);
        }

        *agent.borrow_mut() = Some(ruxc_http_agent_builder(v_http_request)?);
        Ok(true)
    })
}

// Perform HTTP/S request reusing one thread-local agent every time
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

    let debug = unsafe { (*v_http_request).debug };
    let logtype = unsafe { (*v_http_request).logtype };

    if ruxc_http_agent_initialize(v_http_request)? && debug != 0 {
        ruxc_print_log(
            logtype,
            debug,
            3,
            "initializing http agent - reuse on".to_string(),
        );
    }

    let mut retry = unsafe { (*v_http_request).retry };

    HTTPAGENT.with(|agent| -> Result<(), Error> {
        let agent = agent.borrow();
        let agent = agent.as_ref().ok_or_else(|| {
            Error::from(StringError::from(
                "thread-local HTTP agent was not initialized".to_owned(),
            ))
        })?;
        loop {
            ruxc_http_request_perform(
                agent,
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

    Ok(())
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

    let debug = unsafe { (*v_http_request).debug };
    let logtype = unsafe { (*v_http_request).logtype };

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
        ruxc_print_log(logtype, debug, 3, format!("htable key [{htkey}]"));
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
                    format!("initializing http agent for [{htnewkey}]"),
                );
            }
            ht.insert(htnewkey, ruxc_http_agent_builder(v_http_request)?);
        }
        if let Some(agent) = ht.get(&htkey) {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 3, format!("agent retrieved for [{htkey}]"));
            }
            let mut retry = unsafe { (*v_http_request).retry };
            loop {
                ruxc_http_request_perform(
                    agent,
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

    Ok(())
}

fn ruxc_http_request_dispatch(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
    v_method: HTTPMethodType,
) -> Result<(), Error> {
    let reuse = unsafe { (*v_http_request).reuse };
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
/// # Safety
///
/// Both pointers must be valid for reads or writes respectively for the duration of the call.
pub unsafe extern "C" fn ruxc_http_get(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
) -> libc::c_int {
    ruxc_http_request_ffi(v_http_request, v_http_response, HTTPMethodType::MethodGET)
}

// Perform HTTP/S POST request
#[no_mangle]
/// # Safety
///
/// Both pointers must be valid for reads or writes respectively for the duration of the call.
pub unsafe extern "C" fn ruxc_http_post(
    v_http_request: *const RuxcHTTPRequest,
    v_http_response: *mut RuxcHTTPResponse,
) -> libc::c_int {
    ruxc_http_request_ffi(v_http_request, v_http_response, HTTPMethodType::MethodPOST)
}

// Perform HTTP/S DELETE request
#[no_mangle]
/// # Safety
///
/// Both pointers must be valid for reads or writes respectively for the duration of the call.
pub unsafe extern "C" fn ruxc_http_delete(
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
/// # Safety
///
/// Both pointers must be valid for reads or writes respectively for the duration of the call.
pub unsafe extern "C" fn ruxc_http_request(
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
#[path = "ruxc_tests.rs"]
mod tests;

//! RUXC
//!
//! Rust Utilities eXported to C
//!
//! Useful functions written in Rust made available to C
//!   * simple HTTP/S client funtions for GET or POST requests

extern crate libc;

use std;
use rustls;
use webpki;
use url;
use ureq;
use std::collections::HashMap;

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
}

#[repr(C)]
pub struct RuxcHTTPRequest {
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

#[no_mangle]
pub extern "C" fn ruxc_http_response_release(v_http_response: *mut RuxcHTTPResponse)
{
    unsafe {
        if v_http_response.is_null() {
            return;
        }
        if (*v_http_response).resdata.is_null() {
            return;
        }
        std::ffi::CString::from_raw((*v_http_response).resdata)
    };
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
        }
    }
}

impl From<ureq::Error> for Error {
    fn from(source: ureq::Error) -> Self {
        Error {
            source: source.into(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error {
            source: source.into(),
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(source: url::ParseError) -> Self {
        Error {
            source: source.into(),
        }
    }
}

struct AcceptAll {}

impl rustls::ServerCertVerifier for AcceptAll {
    fn verify_server_cert(
        &self,
        _roots: &rustls::RootCertStore,
        _presented_certs: &[rustls::Certificate],
        _dns_name: webpki::DNSNameRef<'_>,
        _ocsp_response: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        Ok(rustls::ServerCertVerified::assertion())
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
        let c_message = std::ffi::CString::new(message).unwrap();
        let c_fmt = std::ffi::CStr::from_bytes_with_nul(b"%s\n\0").expect("format field failed");
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

fn ruxc_http_agent_builder(v_http_request: *const RuxcHTTPRequest)
        -> ureq::AgentBuilder
{
    let v_tlsmode = unsafe { (*v_http_request).tlsmode as i32 };
    let v_timeout_connect = unsafe { (*v_http_request).timeout_connect as u64};
    let v_timeout_read = unsafe { (*v_http_request).timeout_read as u64};
    let v_timeout_write = unsafe { (*v_http_request).timeout_write as u64};
    let v_timeout = unsafe { (*v_http_request).timeout as u64};

    let mut builder = ureq::builder();

    if v_timeout_connect > 0 {
        builder = builder.timeout_connect(std::time::Duration::from_millis(v_timeout_connect))
    }
    if v_timeout_read > 0 {
        builder = builder.timeout_read(std::time::Duration::from_millis(v_timeout_read))
    }
    if v_timeout_write > 0 {
        builder = builder.timeout_write(std::time::Duration::from_millis(v_timeout_write))
    }
    if v_timeout > 0 {
        builder = builder.timeout(std::time::Duration::from_millis(v_timeout));
    }

    if v_tlsmode == 0 {
        let mut client_config = rustls::ClientConfig::new();
        client_config
            .dangerous()
            .set_certificate_verifier(std::sync::Arc::new(AcceptAll {}));
        builder = builder.tls_config(std::sync::Arc::new(client_config));
    }

    return builder;
}

fn ruxc_http_request_perform(
            agent: &ureq::Agent,
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse,
            v_method: &HTTPMethodType)
        -> Result<(), Error>
{
    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    let c_url_str = unsafe {
        std::ffi::CStr::from_ptr((*v_http_request).url)
    };

    let r_url_str = c_url_str.to_str().unwrap();
    let mut req: ureq::Request;

    match *v_method {
        HTTPMethodType::MethodPOST => {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 2, format!("doing HTTP POST - url: {}", r_url_str));
            }
            req = agent.post(r_url_str);
        },
        HTTPMethodType::MethodDELETE => {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 2, format!("doing HTTP DELETE - url: {}", r_url_str));
            }
            req = agent.delete(r_url_str);
        },
        _ => {
            if debug != 0 {
            ruxc_print_log(logtype, debug, 2, format!("doing HTTP GET - url: {}", r_url_str));
            }
            req = agent.get(r_url_str);
        },
    }

    unsafe {
        if !(*v_http_request).headers.is_null() && (*v_http_request).headers_len > 0 {
            let r_headers_str = std::ffi::CStr::from_ptr((*v_http_request).headers).to_str().unwrap();
            if debug != 0 {
                ruxc_print_log(logtype, debug, 3, format!("adding headers: [[{}]]", r_headers_str));
            }
            for line in r_headers_str.lines() {
                let cpos = line.chars().position(|c| c == ':').unwrap();
                if cpos > 0 {
                    req = req.set(&line[0..cpos], &line[(cpos+1)..].trim());
                }
            }
        }
    };

    let res: ureq::Response;
    let exres: std::result::Result<ureq::Response, ureq::Error>;

    if *v_method == HTTPMethodType::MethodPOST {
        let mut r_body_str: &str = "";
        unsafe {
            if !(*v_http_request).data.is_null() && (*v_http_request).data_len > 0 {
                r_body_str = std::ffi::CStr::from_ptr((*v_http_request).data).to_str().unwrap();
            }
        }
        if debug != 0 {
            ruxc_print_log(logtype, debug, 3, format!("post body: [[{}]]", r_body_str));
        }
        exres = req.send_string(r_body_str);
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
        ruxc_print_log(logtype, debug, 3, format!(
            "* ruxc:: {} {} {}",
            res.http_version(),
            res.status(),
            res.status_text()
        ));
    }

    unsafe {
        (*v_http_response).rescode = res.status() as i32;
    };

    let retry = unsafe { (*v_http_request).retry as i32 };

    if retry<=0 || (res.status()>=200 && res.status()<=299) {
        // store body only on no-retry or successful http response
        let body: String = res.into_string()?;

        if debug != 0 {
            ruxc_print_log(logtype, debug, 3, format!("* ruxc:: HTTP response body: {}", body));
        }

        unsafe {
            (*v_http_response).resdata_len = body.chars().count() as i32;
            if (*v_http_response).resdata_len > 0 {
                let c_str_song = std::ffi::CString::new(body).unwrap();
                (*v_http_response).resdata = c_str_song.into_raw();
            }
            (*v_http_response).retcode = 0;
        }
    }

    return Ok(());
}

// Perform HTTP/S request with a new agent every time
fn ruxc_http_request_perform_once(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse,
            v_method: HTTPMethodType)
        -> Result<(), Error>
{
    unsafe {
        (*v_http_response).retcode = -1;
        if (*v_http_request).url.is_null() {
            (*v_http_response).retcode = -20;
            return Ok(());
        }
    };

    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    if debug != 0 {
        ruxc_print_log(logtype, debug, 3, format!("initializing http agent - noreuse"));
    }

    let builder = ruxc_http_agent_builder(v_http_request);

    let agent = builder.build();

    let mut retry = unsafe { (*v_http_request).retry as i32 };

    loop {
        ruxc_http_request_perform(&agent, v_http_request, v_http_response, &v_method).ok();
        if retry<=0 {
            break;
        }
        unsafe {
            if (*v_http_response).rescode>=200 && (*v_http_response).rescode<=299 {
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
            v_method: HTTPMethodType)
        -> Result<(), Error>
{
    unsafe {
        (*v_http_response).retcode = -1;
        if (*v_http_request).url.is_null() {
            (*v_http_response).retcode = -20;
            return Ok(());
        }
    };

    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    let haready = unsafe { HTTPAGENTREADY as u32 };

    if haready == 0 {
        if debug != 0 {
            ruxc_print_log(logtype, debug, 3, format!("initializing http agent - reuse on"));
        }

        let builder = ruxc_http_agent_builder(v_http_request);

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

    HTTPAGENT.with(|agent| {
        loop {
            ruxc_http_request_perform(&(*agent.borrow()), v_http_request, v_http_response, &v_method).ok();
            if retry<=0 {
                break;
            }
            unsafe {
                if (*v_http_response).rescode>=200 && (*v_http_response).rescode<=299 {
                    break;
                }
            }
            retry -= 1;
      }
    });

    return Ok(());
}

// Perform HTTP/S request reusing agents kept in hashmap by base URL
fn ruxc_http_request_perform_hashmap(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse,
            v_method: HTTPMethodType)
        -> Result<(), Error>
{
    unsafe {
        (*v_http_response).retcode = -1;
        if (*v_http_request).url.is_null() {
            (*v_http_response).retcode = -20;
            return Ok(());
        }
    };

    let debug = unsafe { (*v_http_request).debug as i32 };
    let logtype = unsafe { (*v_http_request).logtype as i32 };

    let r_url_str = unsafe {
        std::ffi::CStr::from_ptr((*v_http_request).url).to_string_lossy().into_owned()
    };

    let url = url::Url::parse(&r_url_str)?;

    let htkey = format!("{}://{}:{}", url.scheme(),
                    url.host_str().unwrap_or("127.0.0.1"),
                    url.port_or_known_default().unwrap_or(80));

    if debug != 0 {
        ruxc_print_log(logtype, debug, 3, format!("htable key [{}]", htkey));
    }

    HTTPAGENTMAP.with(|item| {
        let mut ht = item.borrow_mut();
        if ! ht.contains_key(&htkey) {
            let htnewkey = String::clone(&htkey);
            if debug != 0 {
                ruxc_print_log(logtype, debug, 3, format!("initializing http agent for [{}]", htnewkey));
            }
            let builder = ruxc_http_agent_builder(v_http_request);
            ht.insert(htnewkey, builder.build());
        }
        if let Some(agent) = ht.get(&htkey) {
            if debug != 0 {
                ruxc_print_log(logtype, debug, 3, format!("agent retrieved for [{}]", htkey));
            }
            let mut retry = unsafe { (*v_http_request).retry as i32 };
            loop {
                ruxc_http_request_perform(&agent, v_http_request, v_http_response, &v_method).ok();
                if retry<=0 {
                    break;
                }
                unsafe {
                    if (*v_http_response).rescode>=200 && (*v_http_response).rescode<=299 {
                        break;
                    }
                }
                retry -= 1;
          }
        }
    });

    return Ok(());
}

// Perform HTTP/S GET request
#[no_mangle]
pub extern "C" fn ruxc_http_get(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse
        ) -> libc::c_int
{
    let reuse = unsafe { (*v_http_request).reuse as i32 };
    match reuse {
        1 => ruxc_http_request_perform_reuse(v_http_request, v_http_response, HTTPMethodType::MethodGET).ok(),
        2 => ruxc_http_request_perform_hashmap(v_http_request, v_http_response, HTTPMethodType::MethodGET).ok(),
        _ => ruxc_http_request_perform_once(v_http_request, v_http_response, HTTPMethodType::MethodGET).ok(),
    };
    return unsafe { (*v_http_response).retcode };
}

// Perform HTTP/S POST request
#[no_mangle]
pub extern "C" fn ruxc_http_post(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse
        ) -> libc::c_int
{
    let reuse = unsafe { (*v_http_request).reuse as i32 };
    match reuse {
        1 => ruxc_http_request_perform_reuse(v_http_request, v_http_response, HTTPMethodType::MethodPOST).ok(),
        2 => ruxc_http_request_perform_hashmap(v_http_request, v_http_response, HTTPMethodType::MethodPOST).ok(),
        _ => ruxc_http_request_perform_once(v_http_request, v_http_response, HTTPMethodType::MethodPOST).ok(),
    };
    return unsafe { (*v_http_response).retcode };
}

// Perform HTTP/S DELETE request
#[no_mangle]
pub extern "C" fn ruxc_http_delete(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse
        ) -> libc::c_int
{
    let reuse = unsafe { (*v_http_request).reuse as i32 };
    match reuse {
        1 => ruxc_http_request_perform_reuse(v_http_request, v_http_response, HTTPMethodType::MethodDELETE).ok(),
        2 => ruxc_http_request_perform_hashmap(v_http_request, v_http_response, HTTPMethodType::MethodDELETE).ok(),
        _ => ruxc_http_request_perform_once(v_http_request, v_http_response, HTTPMethodType::MethodDELETE).ok(),
    };
    return unsafe { (*v_http_response).retcode };
}
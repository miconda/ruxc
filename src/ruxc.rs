extern crate libc;

use std;

use rustls;

use webpki;

use ureq;

thread_local!(static HTTPAGENT: std::cell::RefCell<ureq::Agent> = std::cell::RefCell::new(ureq::Agent::new()));

static mut HTTPAGENTREADY: u32 = 0;

#[derive(PartialEq)]
enum HTTPMethodType {
    MethodGET,
    MethodPOST,
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

fn ruxc_http_agent_builder(v_http_request: *const RuxcHTTPRequest)
        -> ureq::AgentBuilder
{
    let tlsmode = unsafe { (*v_http_request).tlsmode as i32 };

    let mut builder = ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(unsafe { (*v_http_request).timeout_connect as u64}))
        .timeout_read(std::time::Duration::from_millis(unsafe { (*v_http_request).timeout_read as u64}))
        .timeout_write(std::time::Duration::from_millis(unsafe { (*v_http_request).timeout_write as u64}))
        .timeout(std::time::Duration::from_millis(unsafe { (*v_http_request).timeout as u64}));

    if tlsmode == 0 {
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
            v_method: HTTPMethodType)
        -> Result<(), Error>
{
    let debug = unsafe { (*v_http_request).debug as i32 };

    let c_url_str = unsafe {
        std::ffi::CStr::from_ptr((*v_http_request).url)
    };

    let r_url_str = c_url_str.to_str().unwrap();
    let mut req: ureq::Request;

    if v_method == HTTPMethodType::MethodPOST {
        if debug != 0 {
            println!("* ruxc:: doing HTTP POST - url: {}", r_url_str);
        }
        req = agent.post(r_url_str);
    } else {
        if debug != 0 {
            println!("* ruxc:: doing HTTP GET - url: {}", r_url_str);
        }
        req = agent.get(r_url_str);
    }

    unsafe {
        if !(*v_http_request).headers.is_null() && (*v_http_request).headers_len > 0 {
            let r_headers_str = std::ffi::CStr::from_ptr((*v_http_request).headers).to_str().unwrap();
            for line in r_headers_str.lines() {
                let cpos = line.chars().position(|c| c == ':').unwrap();
                if cpos > 0 {
                    req = req.set(&line[0..cpos], &line[(cpos+1)..].trim());
                }
            }
        }
    };

    let res: ureq::Response;

    if v_method == HTTPMethodType::MethodPOST {
        unsafe {
            if !(*v_http_request).data.is_null() && (*v_http_request).data_len > 0 {
                res = req.send_string(std::ffi::CStr::from_ptr((*v_http_request).data).to_str().unwrap())?;
            } else {
                res = req.send_string("")?;
            }
        }
    } else {
        res = req.call()?;
    }

    if debug != 0 {
        println!(
            "* ruxc:: {} {} {}",
            res.http_version(),
            res.status(),
            res.status_text()
        );
    }

    unsafe {
        (*v_http_response).rescode = res.status() as i32;
    };

    let body: String = res.into_string()?;

    if debug != 0 {
        println!("* ruxc:: HTTP response body: {}", body);
    }

    unsafe {
        (*v_http_response).resdata_len = body.chars().count() as i32;
        if (*v_http_response).resdata_len > 0 {
            let c_str_song = std::ffi::CString::new(body).unwrap();
            (*v_http_response).resdata = c_str_song.into_raw();
        }
        (*v_http_response).retcode = 0;
    }

    return Ok(());
}

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

    if debug != 0 {
        println!("* ruxc:: initializing http agent - noreuse");
    }

    let builder = ruxc_http_agent_builder(v_http_request);

    let agent = builder.build();

    return ruxc_http_request_perform(&agent, v_http_request, v_http_response, v_method);
}

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

    let haready = unsafe { HTTPAGENTREADY as u32 };

    if haready == 0 {
        if debug != 0 {
            println!("* ruxc:: initializing http agent - reuse on");
        }

        let builder = ruxc_http_agent_builder(v_http_request);

        HTTPAGENT.with(|agent| {
            *agent.borrow_mut() = builder.build();
        });
        if debug != 0 {
            println!("* ruxc:: saving ready state - reuse on");
        }
        unsafe {
            HTTPAGENTREADY = 1;
        };
    }

    HTTPAGENT.with(|agent| {
        ruxc_http_request_perform(&(*agent.borrow()), v_http_request, v_http_response, v_method).ok();
    });

    return Ok(());
 }

#[no_mangle]
pub extern "C" fn ruxc_http_get(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse
        ) -> libc::c_int
{
    let reuse = unsafe { (*v_http_request).reuse as i32 };
    if reuse == 1 {
        ruxc_http_request_perform_reuse(v_http_request, v_http_response, HTTPMethodType::MethodGET).ok();
    } else {
        ruxc_http_request_perform_once(v_http_request, v_http_response, HTTPMethodType::MethodGET).ok();
    }
    return unsafe { (*v_http_response).retcode };
}

#[no_mangle]
pub extern "C" fn ruxc_http_post(
            v_http_request: *const RuxcHTTPRequest,
            v_http_response: *mut RuxcHTTPResponse
        ) -> libc::c_int
{
    let reuse = unsafe { (*v_http_request).reuse as i32 };
    if reuse == 1 {
        ruxc_http_request_perform_reuse(v_http_request, v_http_response, HTTPMethodType::MethodPOST).ok();
    } else {
        ruxc_http_request_perform_once(v_http_request, v_http_response, HTTPMethodType::MethodPOST).ok();
    }
    return unsafe { (*v_http_response).retcode };
}
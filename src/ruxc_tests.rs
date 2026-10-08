use super::{
    ruxc_buffer_from_raw_parts, ruxc_http_agent_builder, ruxc_http_agent_initialize, ruxc_http_get,
    ruxc_http_get_max_response_size, ruxc_http_request, ruxc_http_response_read_body,
    ruxc_http_response_release, ruxc_http_response_store_body, ruxc_http_response_storing,
    ruxc_http_set_max_response_size, ruxc_timeout_from_millis, ruxc_utf8_buffer_from_raw_parts,
    RuxcHTTPRequest, RuxcHTTPResponse, RUXC_HTTP_RET_INVALID_ARGUMENT, RUXC_HTTP_RET_INVALID_INPUT,
    RUXC_HTTP_RET_PANIC, RUXC_HTTP_RET_RESPONSE_TOO_LARGE,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Mutex;

static HTTP_TEST_LOCK: Mutex<()> = Mutex::new(());

fn spawn_http_server(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(2)))
                .unwrap();

            let mut request = Vec::new();
            let mut expected_len = None;
            loop {
                let mut chunk = [0_u8; 4096];
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..count]);

                if expected_len.is_none() {
                    if let Some(header_end) =
                        request.windows(4).position(|part| part == b"\r\n\r\n")
                    {
                        let headers = String::from_utf8_lossy(&request[..header_end]);
                        let content_len = headers
                            .lines()
                            .find_map(|line| {
                                line.split_once(':').and_then(|(name, value)| {
                                    name.eq_ignore_ascii_case("content-length")
                                        .then(|| value.trim().parse::<usize>().unwrap())
                                })
                            })
                            .unwrap_or(0);
                        expected_len = Some(header_end + 4 + content_len);
                    }
                }

                if expected_len.is_some_and(|length| request.len() >= length) {
                    break;
                }
            }

            requests.push(request);
            stream.write_all(&response).unwrap();
            stream.flush().unwrap();
        }
        requests
    });

    (format!("http://{address}/resource"), handle)
}

fn response_bytes(response: &RuxcHTTPResponse) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(response.resdata.cast::<u8>(), response.resdata_len as usize)
    }
}

#[test]
fn stores_only_successful_or_final_retry_responses() {
    assert!(ruxc_http_response_storing(200, false));
    assert!(!ruxc_http_response_storing(503, false));
    assert!(ruxc_http_response_storing(503, true));
}

#[test]
fn reads_only_the_declared_buffer_length() {
    let data = b"abcXYZ";
    let value =
        unsafe { ruxc_utf8_buffer_from_raw_parts(data.as_ptr().cast(), 3, "test buffer").unwrap() };

    assert_eq!(value, "abc");
}

#[test]
fn preserves_embedded_nul_bytes() {
    let data = b"a\0b";
    let value = unsafe {
        ruxc_buffer_from_raw_parts(data.as_ptr().cast(), data.len() as i32, "test buffer").unwrap()
    };

    assert_eq!(value, data);
}

#[test]
fn rejects_invalid_buffer_metadata() {
    assert!(unsafe { ruxc_buffer_from_raw_parts(std::ptr::null(), 1, "test buffer") }.is_err());
    assert!(unsafe { ruxc_buffer_from_raw_parts(std::ptr::null(), -1, "test buffer") }.is_err());
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

    unsafe { ruxc_http_response_release(&mut response) };
    assert!(response.resdata.is_null());
    assert_eq!(response.resdata_len, 0);

    unsafe { ruxc_http_response_release(&mut response) };
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

    unsafe { ruxc_http_response_release(&mut response) };
}

#[test]
fn response_body_reader_enforces_the_configured_limit() {
    let exact_limit = b"abcd";
    assert_eq!(
        ruxc_http_response_read_body(std::io::Cursor::new(exact_limit), exact_limit.len()).unwrap(),
        exact_limit
    );

    let err = ruxc_http_response_read_body(std::io::Cursor::new(b"abcde"), 4).unwrap_err();
    assert_eq!(err.retcode, RUXC_HTTP_RET_RESPONSE_TOO_LARGE);
}

#[test]
fn response_body_reader_preserves_binary_data() {
    let body = b"a\0b\xff";
    assert_eq!(
        ruxc_http_response_read_body(std::io::Cursor::new(body), body.len()).unwrap(),
        body
    );
}

#[test]
fn agent_configuration_preserves_request_semantics() {
    let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
    request.tlsmode = 1;
    request.timeout = 10;
    request.timeout_connect = 20;
    request.timeout_read = 30;
    request.timeout_write = 40;

    let agent = ruxc_http_agent_builder(&request).unwrap();
    let config = agent.config();
    let timeouts = config.timeouts();

    assert!(!config.http_status_as_error());
    assert!(config.allow_non_standard_methods());
    assert_eq!(config.max_redirects(), 5);
    assert!(config.proxy().is_none());
    assert!(!config.tls_config().disable_verification());
    assert_eq!(timeouts.global, Some(std::time::Duration::from_millis(10)));
    assert_eq!(timeouts.connect, Some(std::time::Duration::from_millis(20)));
    assert_eq!(
        timeouts.recv_response,
        Some(std::time::Duration::from_millis(30))
    );
    assert_eq!(
        timeouts.recv_body,
        Some(std::time::Duration::from_millis(30))
    );
    assert_eq!(
        timeouts.send_request,
        Some(std::time::Duration::from_millis(40))
    );
    assert_eq!(
        timeouts.send_body,
        Some(std::time::Duration::from_millis(40))
    );

    request.tlsmode = 0;
    let insecure_agent = ruxc_http_agent_builder(&request).unwrap();
    assert!(insecure_agent.config().tls_config().disable_verification());
}

#[test]
fn retries_and_stores_the_final_non_success_response() {
    let _lock = HTTP_TEST_LOCK.lock().unwrap();
    let (url, server) = spawn_http_server(vec![
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 5\r\nConnection: close\r\n\r\nfirst"
            .to_vec(),
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 6\r\nConnection: close\r\n\r\nsecond"
            .to_vec(),
    ]);
    let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
    let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
    request.url = url.as_ptr().cast();
    request.url_len = url.len() as i32;
    request.tlsmode = 1;
    request.retry = 1;

    assert_eq!(unsafe { ruxc_http_get(&request, &mut response) }, 0);
    assert_eq!(response.rescode, 503);
    assert_eq!(response_bytes(&response), b"second");
    unsafe { ruxc_http_response_release(&mut response) };

    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn custom_method_preserves_binary_body_and_last_header_value() {
    let _lock = HTTP_TEST_LOCK.lock().unwrap();
    let (url, server) = spawn_http_server(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\na\0b".to_vec(),
    ]);
    let method = b"PROPFIND\0";
    let headers = b"X-Test: first\r\nX-Test: second\r\n";
    let body = b"x\0y";
    let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
    let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
    request.method = method.as_ptr().cast();
    request.url = url.as_ptr().cast();
    request.url_len = url.len() as i32;
    request.headers = headers.as_ptr().cast();
    request.headers_len = headers.len() as i32;
    request.data = body.as_ptr().cast();
    request.data_len = body.len() as i32;
    request.tlsmode = 1;

    assert_eq!(unsafe { ruxc_http_request(&request, &mut response) }, 0);
    assert_eq!(response.rescode, 200);
    assert_eq!(response_bytes(&response), b"a\0b");
    unsafe { ruxc_http_response_release(&mut response) };

    let requests = server.join().unwrap();
    let request_bytes = &requests[0];
    let headers_end = request_bytes
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap();
    let request_headers = String::from_utf8_lossy(&request_bytes[..headers_end]);
    assert!(request_headers.starts_with("PROPFIND /resource HTTP/1.1\r\n"));
    assert!(request_headers.contains("x-test: second"));
    assert!(!request_headers.contains("x-test: first"));
    assert_eq!(&request_bytes[(headers_end + 4)..], body);
}

#[test]
fn oversized_http_response_returns_the_stable_error_code() {
    let _lock = HTTP_TEST_LOCK.lock().unwrap();
    let original_limit = ruxc_http_get_max_response_size();
    assert_eq!(ruxc_http_set_max_response_size(4), 0);

    let (url, server) = spawn_http_server(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nabcde".to_vec(),
    ]);
    let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
    let mut response: RuxcHTTPResponse = unsafe { std::mem::zeroed() };
    request.url = url.as_ptr().cast();
    request.url_len = url.len() as i32;
    request.tlsmode = 1;

    assert_eq!(
        unsafe { ruxc_http_get(&request, &mut response) },
        RUXC_HTTP_RET_RESPONSE_TOO_LARGE
    );
    assert_eq!(response.rescode, 200);
    assert!(response.resdata.is_null());
    assert_eq!(response.resdata_len, 0);
    assert_eq!(ruxc_http_set_max_response_size(original_limit), 0);
    server.join().unwrap();
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
        unsafe { ruxc_http_request(std::ptr::null(), &mut response) },
        RUXC_HTTP_RET_INVALID_ARGUMENT
    );
    assert_eq!(response.retcode, RUXC_HTTP_RET_INVALID_ARGUMENT);
    assert_eq!(response.rescode, 0);
    assert!(response.resdata.is_null());
    assert_eq!(response.resdata_len, 0);
    assert_eq!(
        unsafe { ruxc_http_request(std::ptr::null(), std::ptr::null_mut()) },
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
        unsafe { ruxc_http_request(&request, &mut response) },
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
        unsafe { super::ruxc_http_get(&request, &mut response) },
        RUXC_HTTP_RET_INVALID_INPUT
    );
    assert_eq!(response.retcode, RUXC_HTTP_RET_INVALID_INPUT);
}

#[test]
fn reuse_agents_are_initialized_independently_per_thread() {
    super::HTTPAGENT.with(|agent| {
        *agent.borrow_mut() = None;
    });

    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (1..=8)
        .map(|timeout| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                super::HTTPAGENT.with(|agent| assert!(agent.borrow().is_none()));
                barrier.wait();

                let mut request: RuxcHTTPRequest = unsafe { std::mem::zeroed() };
                request.tlsmode = 1;
                request.timeout = timeout;

                assert!(ruxc_http_agent_initialize(&request).unwrap());
                assert!(!ruxc_http_agent_initialize(&request).unwrap());
                super::HTTPAGENT.with(|agent| assert!(agent.borrow().is_some()));
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }
    super::HTTPAGENT.with(|agent| assert!(agent.borrow().is_none()));
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
            unsafe { super::ruxc_http_get(&request, &mut response) },
            RUXC_HTTP_RET_PANIC
        );
    });
    assert_eq!(response.retcode, RUXC_HTTP_RET_PANIC);
}

use super::{
    ruxc_buffer_from_raw_parts, ruxc_http_agent_initialize, ruxc_http_request,
    ruxc_http_response_read_body, ruxc_http_response_release, ruxc_http_response_store_body,
    ruxc_http_response_storing, ruxc_timeout_from_millis, ruxc_utf8_buffer_from_raw_parts,
    RuxcHTTPRequest, RuxcHTTPResponse, RUXC_HTTP_RET_INVALID_ARGUMENT, RUXC_HTTP_RET_INVALID_INPUT,
    RUXC_HTTP_RET_PANIC, RUXC_HTTP_RET_RESPONSE_TOO_LARGE,
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
            super::ruxc_http_get(&request, &mut response),
            RUXC_HTTP_RET_PANIC
        );
    });
    assert_eq!(response.retcode, RUXC_HTTP_RET_PANIC);
}


#ifndef __LIBRUXC_H__
#define __LIBRUXC_H__

/* library version string - x.y.z */
#define LIBRUXC_VERSION_STR "1.1.0"
/* library version id - 3 digits for each x.y.z */
#define LIBRUXC_VERSION_ID 1001000

/* HTTP request return codes */
#define RUXC_HTTP_RET_OK 0
#define RUXC_HTTP_RET_ERROR -1
#define RUXC_HTTP_RET_INVALID_ARGUMENT -20
#define RUXC_HTTP_RET_INVALID_INPUT -21
#define RUXC_HTTP_RET_PANIC -99

typedef struct RuxcHTTPRequest {
    char* method;        /* Method */
    char* url;           /* HTTP/S URL */
    int url_len;         /* HTTP/S URL length */
    char* headers;       /* Extra headers separated by \r\n */
    int headers_len;     /* Length of extra headers */
    char* data;          /* Data to be set as HTTP POST body */
    int data_len;        /* Length of data */
    int timeout;         /* Timeout in milliseconds */
    int timeout_connect; /* Connect timeout in milliseconds */
    int timeout_read;    /* Read timeout in milliseconds */
    int timeout_write;   /* Write timeout in milliseconds */
    int tlsmode;         /* TLS mode: 0 - accept all certs; 1 - accept only trusted certs */
    int flags;           /* Internal flags - not in use yet */
    int debug;           /* Debug mode: 0 - no debug; 1 - error; 2 - info; 3 - debug */
    int reuse;           /* Reuse connection mode: 0 - do not reuse;
                          *   1 - single connection; 2 - connections hashmap */
    int retry;           /* How many tries to attempt if not getting 200ok */
    int logtype;         /* Log type: 0 - stdout; 1 - syslog */
} RuxcHTTPRequest;

typedef struct RuxcHTTPResponse {
    int retcode;         /* return code of processing the request */
    int rescode;         /* HTTP response code */
    char* resdata;       /* HTTP response data (body) */
    int resdata_len;     /* Length of response data */
} RuxcHTTPResponse;

/* Release resdata before reusing a response that contains a previous result.
 * Request functions initialize all response fields before performing a request. */

/**
 * Perform a HTTP/S GET request
 */
extern int ruxc_http_get(RuxcHTTPRequest *v_http_request,
		RuxcHTTPResponse *v_http_response);
/**
 * Perform a HTTP/S POST request
 */
extern int ruxc_http_post(RuxcHTTPRequest *v_http_request,
		RuxcHTTPResponse *v_http_response);
/**
 * Perform a HTTP/S DELETE request
 */
extern int ruxc_http_delete(RuxcHTTPRequest *v_http_request,
		RuxcHTTPResponse *v_http_response);

/**
 * Perform a HTTP/S CUSTOM request
 */
extern int ruxc_http_request(RuxcHTTPRequest *v_http_request,
		RuxcHTTPResponse *v_http_response);

/**
 * Release resources associated with a HTTP/S response
 */
extern void ruxc_http_response_release(RuxcHTTPResponse *v_http_response);

#endif

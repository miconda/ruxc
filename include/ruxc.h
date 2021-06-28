
#ifndef __LIBRUXC_H__
#define __LIBRUXC_H__

typedef struct RuxcHTTPRequest {
    char* url;
    int url_len;
    char* headers;
    int headers_len;
    char* data;
    int data_len;
    int timeout;
    int timeout_connect;
    int timeout_read;
    int timeout_write;
    int flags;
} RuxcHTTPRequest;

typedef struct RuxcHTTPResponse {
    int retcode;
    int rescode;
    char* resdata;
    int resdata_len;
} RuxcHTTPResponse;

extern void ruxc_http_response_release(RuxcHTTPResponse *v_http_response);
extern int ruxc_http_get(RuxcHTTPRequest *v_http_request,
		RuxcHTTPResponse *v_http_response);

#endif

#include <stddef.h>
#include <stdint.h>

#include <ruxc.h>

#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(RuxcHTTPRequest) == 96, "RuxcHTTPRequest ABI size changed");
_Static_assert(offsetof(RuxcHTTPRequest, method) == 0, "method ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, url) == 8, "url ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, url_len) == 16, "url_len ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, headers) == 24, "headers ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, data) == 40, "data ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, logtype) == 88, "logtype ABI offset changed");

_Static_assert(sizeof(RuxcHTTPResponse) == 24, "RuxcHTTPResponse ABI size changed");
_Static_assert(offsetof(RuxcHTTPResponse, retcode) == 0, "retcode ABI offset changed");
_Static_assert(offsetof(RuxcHTTPResponse, rescode) == 4, "rescode ABI offset changed");
_Static_assert(offsetof(RuxcHTTPResponse, resdata) == 8, "resdata ABI offset changed");
_Static_assert(offsetof(RuxcHTTPResponse, resdata_len) == 16, "resdata_len ABI offset changed");
#elif UINTPTR_MAX == UINT32_MAX
_Static_assert(sizeof(RuxcHTTPRequest) == 68, "RuxcHTTPRequest ABI size changed");
_Static_assert(offsetof(RuxcHTTPRequest, method) == 0, "method ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, url) == 4, "url ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, url_len) == 8, "url_len ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, headers) == 12, "headers ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, data) == 20, "data ABI offset changed");
_Static_assert(offsetof(RuxcHTTPRequest, logtype) == 64, "logtype ABI offset changed");

_Static_assert(sizeof(RuxcHTTPResponse) == 16, "RuxcHTTPResponse ABI size changed");
_Static_assert(offsetof(RuxcHTTPResponse, retcode) == 0, "retcode ABI offset changed");
_Static_assert(offsetof(RuxcHTTPResponse, rescode) == 4, "rescode ABI offset changed");
_Static_assert(offsetof(RuxcHTTPResponse, resdata) == 8, "resdata ABI offset changed");
_Static_assert(offsetof(RuxcHTTPResponse, resdata_len) == 12, "resdata_len ABI offset changed");
#else
#error "Unsupported pointer width for the ruxc ABI test"
#endif

static int (*get_fn)(RuxcHTTPRequest *, RuxcHTTPResponse *) = ruxc_http_get;
static int (*post_fn)(RuxcHTTPRequest *, RuxcHTTPResponse *) = ruxc_http_post;
static int (*delete_fn)(RuxcHTTPRequest *, RuxcHTTPResponse *) = ruxc_http_delete;
static int (*request_fn)(RuxcHTTPRequest *, RuxcHTTPResponse *) = ruxc_http_request;
static void (*release_fn)(RuxcHTTPResponse *) = ruxc_http_response_release;

int main(void)
{
	(void)get_fn;
	(void)post_fn;
	(void)delete_fn;
	(void)request_fn;
	(void)release_fn;

	if(ruxc_http_get_max_response_size() != RUXC_HTTP_DEFAULT_MAX_RESPONSE_SIZE) {
		return 1;
	}
	return ruxc_http_set_max_response_size(RUXC_HTTP_DEFAULT_MAX_RESPONSE_SIZE);
}

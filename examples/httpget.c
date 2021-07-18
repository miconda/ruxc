/**
 * build on macos:
 *   cd ..; gcc -o examples/httpget -I include/ examples/httpget.c target/release/libruxc.a -framework Security
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <ruxc.h>

int main(int argc, char *argv[])
{
	RuxcHTTPRequest v_http_request = {0};
	RuxcHTTPResponse v_http_response = {0};

    v_http_request.timeout = 5000;
    v_http_request.timeout_connect = 5000;
    v_http_request.timeout_read = 5000;
    v_http_request.timeout_write = 5000;

	v_http_request.url = "https://www.kamailio.org/pub/kamailio/latest-stable-version-number";
	v_http_request.url_len = strlen(v_http_request.url);

	v_http_request.headers = "X-My-Key: abcdefgh\r\nX-Info: request\r\n";
	v_http_request.headers_len = strlen(v_http_request.headers);

	v_http_request.debug = 1;

	ruxc_http_get(&v_http_request, &v_http_response);

	if(v_http_response.retcode < 0) {
		printf("* c:: failed to perform http get - retcode: %d\n", v_http_response.retcode);
	} else {
		if(v_http_response.resdata != NULL &&  v_http_response.resdata_len>0) {
			printf("* c:: response code: %d - data len: %d - data: [%.*s]\n",
					v_http_response.rescode, v_http_response.resdata_len,
					v_http_response.resdata_len, v_http_response.resdata);
		}
	}
	ruxc_http_response_release(&v_http_response);
	return 0;
}

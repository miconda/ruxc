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
    v_http_request.timeout_write = 500;

	v_http_request.url = "https://www.kamailio.org/pub/kamailio/latest-stable-version-number";
	v_http_request.url_len = strlen(v_http_request.url);

	ruxc_http_get(&v_http_request, &v_http_response);

	if(v_http_response.retcode < 0) {
		printf("failed to perform http get - retcode: %d\n", v_http_response.retcode);
	} else {
		if(v_http_response.resdata != NULL &&  v_http_response.resdata_len>0) {
			printf("response data len: %d - data: [%.*s]\n", v_http_response.resdata_len,
					v_http_response.resdata_len, v_http_response.resdata);
		}
	}
	ruxc_http_response_release(&v_http_response);
	return 0;
}

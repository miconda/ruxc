/**
 * build on macos:
 *   cd ..; gcc -o examples/httpget -I include/ examples/httpget.c target/release/libruxc.a -framework Security
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/time.h>

#include <ruxc.h>

int main(int argc, char *argv[])
{
	RuxcHTTPRequest v_http_request = {0};
	RuxcHTTPResponse v_http_response = {0};
	char *url_list[] = {
		"https://www.kamailio.org/pub/kamailio/latest-stable-version-number",
		"https://www.kamailio.org/pub/kamailio/latest-stable-version-number",
		"https://www.kamailio.org/pub/kamailio/latest/README",
		"https://www.kamailio.org/pub/kamailio/latest/README",
		"https://www.kamailio.org/pub/kamailio/5.5.0/README",
		"https://www.kamailio.org/pub/kamailio/5.5.0/README",
		NULL
	};
	int i = 0;
	char hdrbuf[256];
	struct timeval tvb = {0}, tve = {0};
	unsigned int diff = 0;

    v_http_request.timeout = 5000;
    v_http_request.timeout_connect = 5000;
    v_http_request.timeout_read = 5000;
    v_http_request.timeout_write = 5000;
	v_http_request.debug = 1;
	v_http_request.reuse = 1;

	for(i = 0; url_list[i]!=NULL; i++) {
		v_http_request.url = url_list[i];
		v_http_request.url_len = strlen(v_http_request.url);

		snprintf(hdrbuf, 255, "X-My-Key: KEY-%d\r\nX-Info: REQUEST-%d\r\n", i, i);
		v_http_request.headers = hdrbuf;
		v_http_request.headers_len = strlen(v_http_request.headers);

		gettimeofday(&tvb, NULL);
		ruxc_http_get(&v_http_request, &v_http_response);
		gettimeofday(&tve, NULL);
		diff = (tve.tv_sec - tvb.tv_sec) * 1000000 + (tve.tv_usec - tvb.tv_usec);
		printf("* c:: http request[%d] done in: %u usec\n", i, diff);


		if(v_http_response.retcode < 0) {
			printf("* c:: failed to perform http get [%d] - retcode: %d\n", i, v_http_response.retcode);
		} else {
			if(v_http_response.resdata != NULL &&  v_http_response.resdata_len>0) {
				printf("* c:: response [%d] code: %d - data len: %d - data: [%.*s]\n", i,
						v_http_response.rescode, v_http_response.resdata_len,
						v_http_response.resdata_len, v_http_response.resdata);
			}
		}
		ruxc_http_response_release(&v_http_response);

		v_http_response.resdata = NULL;
		v_http_response.resdata_len = 0;
		v_http_response.retcode = 0;
		v_http_response.rescode = 0;
	}

	return 0;
}

/**
 * minimal http client using libruxc
 *
 * build on macos:
 *   cd ..; gcc -o examples/httpcli -I include/ examples/httpcli.c target/release/libruxc.a -framework Security
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <stdint.h>
#include <unistd.h>
#include <time.h>
#include <sys/time.h>
#include <syslog.h>

#include <ruxc.h>

static char *version = "httpcli 0.2.0";

static char* helpmsg = "\
Usage: httpcli [params]\n\
Options:\n\
    -a count      number of retry attempts\n\
    -b data       http request body data\n\
    -d level      debug level (0 - no logs; 1 - errors; 2 - info; 3 - debug)\n\
    -H header     add HTTP header 'Name: value' (repeatable)\n\
    -i            accept HTTPS certificates not signed by a trusted CA\n\
    -l type       log type (0 - print to stdout; 1 - print to syslog)\n\
    -m method     method value\n\
    -n count      number of times to request each URL\n\
    -P            do http POST instead of GET\n\
    -r mode       reuse mode\n\
    -t msec       timeout in milliseconds\n\
    -u url        URL to request (repeatable)\n\
    -h            this help message\n\
";

static int append_header(char **headers, size_t *headers_len, const char *header)
{
	const char *separator = strchr(header, ':');
	size_t header_len = strlen(header);
	size_t new_len;
	char *new_headers;

	if(separator == NULL || separator == header || strchr(header, '\r') != NULL
			|| strchr(header, '\n') != NULL) {
		return -1;
	}
	if(header_len > (size_t)INT_MAX - 2
			|| *headers_len > (size_t)INT_MAX - header_len - 2) {
		return -1;
	}

	new_len = *headers_len + header_len + 2;
	new_headers = realloc(*headers, new_len + 1);
	if(new_headers == NULL) {
		return -1;
	}

	memcpy(new_headers + *headers_len, header, header_len);
	memcpy(new_headers + *headers_len + header_len, "\r\n", 2);
	new_headers[new_len] = '\0';
	*headers = new_headers;
	*headers_len = new_len;
	return 0;
}

static int append_url(char ***urls, size_t *url_count, const char *url)
{
	char **new_urls;

	if(url == NULL || url[0] == '\0'
			|| *url_count == SIZE_MAX / sizeof(**urls)) {
		return -1;
	}

	new_urls = realloc(*urls, (*url_count + 1) * sizeof(**urls));
	if(new_urls == NULL) {
		return -1;
	}

	new_urls[*url_count] = (char *)url;
	*urls = new_urls;
	(*url_count)++;
	return 0;
}

int main(int argc, char *argv[])
{
	RuxcHTTPRequest v_http_request = {0};
	RuxcHTTPResponse v_http_response = {0};
	char **url_list = NULL;
	size_t url_count = 0;
	size_t request_count;
	size_t request_index;
	int post = 0;
	int c = 0;
	int ncount = 1;
	int i = 0;
	int retry = 0;
	char hdrbuf[256];
	struct timeval tvb = {0}, tve = {0};
	unsigned int diff = 0;
	int logtype = 0;
	int debug = 0;
	int timeout = 5000;
	int tlsmode = 1;
	char *bodydata = "{ \"info\": \"testing\", \"id\": 80 }";
	int reuse = 0;
	char *method = NULL;
	char *headers = NULL;
	size_t headers_len = 0;

	opterr=0;
	while ((c=getopt(argc, argv, "a:b:d:H:il:m:n:Pr:t:u:h")) != -1) {
		i++;
		switch(c) {
			case 'a':
				retry = atoi(optarg);
				if(retry<0) { retry = 0; }
				break;
			case 'r':
				reuse = atoi(optarg);
				if(reuse<0 || reuse>2) { reuse = 0; }
				break;
			case 'l':
				logtype = atoi(optarg);
				if(logtype<0 || logtype>1) { logtype = 0; }
				break;
			case 'd':
				debug = atoi(optarg);
				if(debug<0 || debug>3) { debug = 0; }
				break;
			case 'H':
				if(append_header(&headers, &headers_len, optarg) < 0) {
					fprintf(stderr, "invalid HTTP header: %s\n", optarg);
					free(headers);
					free(url_list);
					return 1;
				}
				break;
			case 'i':
				tlsmode = 0;
				break;
			case 'm':
				method = optarg;
				break;
			case 'n':
				ncount = atoi(optarg);
				if(ncount<=0) { ncount = 1; }
				break;
			case 'P':
				post = 1;
				break;
			case 'b':
				bodydata = optarg;
				break;
			case 't':
				timeout = atoi(optarg);
				if(timeout<=0) { timeout = 5000; }
				break;
			case 'u':
				if(append_url(&url_list, &url_count, optarg) < 0) {
					fprintf(stderr, "invalid URL or too many URLs: %s\n", optarg);
					free(headers);
					free(url_list);
					return 1;
				}
				break;
			case 'h':
				printf("version: %s\n", version);
				printf("%s", helpmsg);
				free(headers);
				free(url_list);
				return 0;
			default:
				printf("unknown cli option %c (%d) (i: %d)\n", (char)c, c, i);
				free(headers);
				free(url_list);
				return 1;
		}
	}

	if(url_count == 0) {
		fprintf(stderr, "at least one URL must be provided with -u\n");
		free(headers);
		free(url_list);
		return 1;
	}
	if((size_t)ncount > SIZE_MAX / url_count) {
		fprintf(stderr, "too many requests\n");
		free(headers);
		free(url_list);
		return 1;
	}
	request_count = (size_t)ncount * url_count;

	if(debug==2) {
		openlog(argv[0], LOG_PID, LOG_DAEMON);
	}
	v_http_request.method = method;
	v_http_request.timeout = timeout;
	v_http_request.timeout_connect = timeout;
	v_http_request.timeout_read = timeout;
	v_http_request.timeout_write = timeout;
	v_http_request.tlsmode = tlsmode;
	v_http_request.logtype = logtype;
	v_http_request.debug = debug;
	v_http_request.reuse = reuse;
	v_http_request.retry = retry;

	for(request_index = 0; request_index < request_count; request_index++) {
		printf("\n* c:: request %zu =========================\n\n", request_index);
		v_http_request.url = url_list[request_index % url_count];
		v_http_request.url_len = strlen(v_http_request.url);
		printf("\n* c:: request type=%s url=%s\n", (method!=NULL)?method:((post==1)?"post":"get"), v_http_request.url);

		if(headers != NULL) {
			v_http_request.headers = headers;
			v_http_request.headers_len = (int)headers_len;
		} else {
			snprintf(hdrbuf, 255, "X-My-Key: KEY-%zu\r\nX-Info: REQUEST-%zu\r\n",
					request_index, request_index);
			v_http_request.headers = hdrbuf;
			v_http_request.headers_len = strlen(v_http_request.headers);
		}

		if(method != NULL) {
			if(bodydata != NULL && strlen(bodydata) > 0) {
				v_http_request.data = bodydata;
				v_http_request.data_len = strlen(v_http_request.data);
			}
			gettimeofday(&tvb, NULL);
			ruxc_http_request(&v_http_request, &v_http_response);
			gettimeofday(&tve, NULL);
		} else if(post==1) {
			v_http_request.data = bodydata;
			v_http_request.data_len = strlen(v_http_request.data);
			gettimeofday(&tvb, NULL);
			ruxc_http_post(&v_http_request, &v_http_response);
			gettimeofday(&tve, NULL);
		} else {
			gettimeofday(&tvb, NULL);
			ruxc_http_get(&v_http_request, &v_http_response);
			gettimeofday(&tve, NULL);
		}
		diff = (tve.tv_sec - tvb.tv_sec) * 1000000 + (tve.tv_usec - tvb.tv_usec);
		printf("* c:: http request[%zu] done in: %u usec\n", request_index, diff);


		if(v_http_response.retcode < 0) {
			printf("* c:: failed to perform http get [%zu] - retcode: %d - rescode: %d\n",
					request_index, v_http_response.retcode, v_http_response.rescode);
		} else {
			if(v_http_response.resdata != NULL &&  v_http_response.resdata_len>0) {
				printf("* c:: response [%zu] code: %d - data len: %d - data: [%.*s]\n",
						request_index,
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
	printf("\n");
	if(debug==2) {
		closelog();
	}
	free(headers);
	free(url_list);
	return 0;
}

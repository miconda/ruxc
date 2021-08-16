/**
 * minimal http client using libruxc
 *
 * build on macos:
 *   cd ..; gcc -o examples/httpcli -I include/ examples/httpcli.c target/release/libruxc.a -framework Security
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <time.h>
#include <sys/time.h>
#include <syslog.h>

#include <ruxc.h>

static char *version = "httpcli 0.1.0";
static char* helpmsg = "\
Usage: httpcli [params]\n\
Options:\n\
    -a count      number of retry attempts\n\
    -d level      debug level (0 - no logs; 1 - errors; 2 - debug)\n\
    -l type       log type (0 - print to stdout; 1 - print to syslog)\n\
    -n count      number of requests to be sent\n\
    -p            do http post instead of get\n\
    -P data       http post data\n\
    -r mode       reuse mode\n\
    -t usec       microseconds timeout\n\
    -u url        URL to request\n\
    -h            this help message\n\
";

int main(int argc, char *argv[])
{
	RuxcHTTPRequest v_http_request = {0};
	RuxcHTTPResponse v_http_response = {0};
	char *url_list[] = {
		"http://www.kamailio.org/pub/kamailio/latest-stable-version-number",
		"https://www.kamailio.org/pub/kamailio/latest-stable-version-number",
		"http://www.kamailio.org/pub/kamailio/latest/README",
		"https://www.kamailio.org/pub/kamailio/latest/README",
		"http://www.kamailio.org/pub/kamailio/5.5.0/README",
		"https://www.kamailio.org/pub/kamailio/5.5.0/README",
		NULL
	};
	int post = 0;
	char c = 0;
	int ncount = 1;
	int i = 0;
	int retry = 0;
	char hdrbuf[256];
	struct timeval tvb = {0}, tve = {0};
	unsigned int diff = 0;
	int logtype = 0;
	int debug = 0;
	int timeout = 5000;
	char *postdata = "{ \"info\": \"testing\", \"id\": 80 }";
	int reuse = 0;
	char *url = NULL;

	opterr=0;
	while ((c=getopt(argc,argv, "a:d:l:n:P:r:t:u:hp"))!=-1){
		switch(c){
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
				if(debug<0 || debug>2) { debug = 0; }
				break;
			case 'p':
				post = 1;
				break;
			case 'P':
				postdata = optarg;
				break;
			case 't':
				timeout = atoi(optarg);
				if(timeout<=0) { timeout = 5000; }
				break;
			case 'u':
				url = optarg;
				break;
			case 'h':
				printf("version: %s\n", version);
				printf("%s", helpmsg);
				exit(0);
				break;
			default:
				printf("unknown cli option %c\n", c);
				exit(-1);
		}
	}

	if(debug==2) {
		openlog(argv[0], LOG_PID, LOG_DAEMON);
	}
	v_http_request.timeout = timeout;
	v_http_request.timeout_connect = timeout;
	v_http_request.timeout_read = timeout;
	v_http_request.timeout_write = timeout;
	v_http_request.logtype = logtype;
	v_http_request.debug = debug;
	v_http_request.reuse = reuse;
	v_http_request.retry = retry;

	for(i = 0; i<ncount; i++) {
		printf("\n* c:: request %d =========================\n\n", i);
		if(url!=NULL) {
			v_http_request.url = url;
		} else {
			if(url_list[i]!=NULL) {
				v_http_request.url = url_list[i];
			} else {
				break;
			}
		}
		v_http_request.url_len = strlen(v_http_request.url);
		printf("\n* c:: request type=%s url=%s\n", (post==1)?"post":"get", v_http_request.url);

		snprintf(hdrbuf, 255, "X-My-Key: KEY-%d\r\nX-Info: REQUEST-%d\r\n", i, i);
		v_http_request.headers = hdrbuf;
		v_http_request.headers_len = strlen(v_http_request.headers);

		if(post==1) {
			v_http_request.data = postdata;
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
		printf("* c:: http request[%d] done in: %u usec\n", i, diff);


		if(v_http_response.retcode < 0) {
			printf("* c:: failed to perform http get [%d] - retcode: %d - rescode: %d\n",
					i, v_http_response.retcode, v_http_response.rescode);
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
	printf("\n");
	if(debug==2) {
		closelog();
	}
	return 0;
}

# RUXC #

`RUst eXports to C`

C/C++ library exporting useful functions written in Rust.

First group of functions offer a simple HTTP client library, with functions
for doing GET, POST or DELETE requests. It has support for HTTPS.

## Build ##

### Build With Cargo ###

The library can be build directly using `cargo` tool that comes with `rust`:

```
git clone https://github.com/miconda/ruxc
cd ruxc
cargo build --release
```

After build, the `libruxc` static (`.a`) and dynamic (`.so` or `.dylib`) library
files are in the folder `target/release/`.

The C structures and functions are available in `include/ruxc.h` file.

### Build With Make ###

For convenience, a `Makefile` is available to simplify the build and install
tasks:

```
git clone https://github.com/miconda/ruxc
cd ruxc
make lib
make install
```

## C API Functions ##

### HTTP Client Functions ###

The library offers HTTP client functions to perform blocking GET or POST requests,
with option to reuse connections per process. The functions are built on `ureq` Rust
library (https://github.com/algesten/ureq/), which has also support for HTTPS
based on `rustls` (https://github.com/ctz/rustls).

They are a simple alternative to the versatile cURL library, not depending
on `libssl` or `gnutls` either. The functions are useful when willing to build
a simple HTTP client in C, specially for multi-process applications when one
wants to re-use the HTTP/S connection per process and do not care about
multi-threading constraints of `libssl` or `libcurl`.

It supports setting custom headers for GET and POST requests as well as body data
for POST requests. There is no user/password authentication support. An option
can be set to accept only trusted TLS certificates for HTTPS connections or any
certificate.

For usage example, see also `examples/httpcli.c` or the `ruxc` module of Kamailio
project (https://github.com/kamailio/kamailio/tree/master/src/modules/ruxc).

### Example Of Usage ###

Basic C HTTP GET client:

```c
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
	v_http_request.debug = 1;
	v_http_request.reuse = 1;

	v_http_request.url = "https://myserver.com/get";
	v_http_request.url_len = strlen(v_http_request.url);

	v_http_request.headers = "X-My-Key: xyzw\r\nX-My-Hdr: abcd\r\n";
	v_http_request.headers_len = strlen(v_http_request.headers);

	ruxc_http_get(&v_http_request, &v_http_response);

	if(v_http_response.retcode < 0) {
		printf("failed to perform http get - retcode: %d\n", v_http_response.retcode);
	} else {
		if(v_http_response.resdata != NULL &&  v_http_response.resdata_len>0) {
			printf("http response code: %d - data len: %d - data: [%.*s]\n",
					v_http_response.rescode, v_http_response.resdata_len,
					v_http_response.resdata_len, v_http_response.resdata);
		} else {
			printf("http response code: %d\n", v_http_response.rescode);
		}
	}
	ruxc_http_response_release(&v_http_response);

	return 0;
}
```

Running `examples/httpcli` on MacOS:

```
git clone https://github.com/miconda/ruxc
cd ruxc
cargo build --release
gcc -o examples/httpcli -I include/ examples/httpcli.c target/release/libruxc.a -framework Security
./examples/httpcli
```

## License ##

MIT

**Copyright**: `Daniel-Constantin Mierla <miconda@gmail.com>`

## Credits ##

This library is built using:

  * rust - https://www.rust-lang.org/
  * ureq - https://github.com/algesten/ureq/
  * rustls - https://github.com/ctz/rustls

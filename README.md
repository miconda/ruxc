# ruxc #

`RUst eXports to C`

C library exporting useful functions written in Rust, such as simple HTTP client
functions for doing GET or POST requests.

## Build ##

```
git clone https://github.com/miconda/ruxc
cd ruxc
cargo build --release
```

After build, the `libruxc` static (`.a`) and dynamic (`.so` or `.dynlib`) library
files are in the folder `target/release/`.

The C structures and functions are available in `include/ruxc.h` file.

## API Functions ##

### HTTP Client Functions ###

The library offers HTTP client functions to perform blocking GET or POST requests,
with option to reuse connections per process. The functions are built on `ureq` Rust
library (https://github.com/algesten/ureq/), which has also support for HTTPS
based on `rustls` (https://github.com/ctz/rustls).

They are a simple alternative to the versatile cURL library, not depending
on `libssl` or `gnutls` either. The functions are useful when willing to build
a simple HTTP client in C, specially for multi-process applications when one
wants to re-user the HTTP/S connection per process.

It supports setting custom headers for GET and POST requests as well as body data
for POST requests. There is no user/password authentication support. An option
can be set to accept only trusted TLS certificates for HTTPS connections or any
certificate.

For usage example, see `examples/httpcli.c` or the `ruxc` module of Kamailio
project (https://github.com/kamailio/kamailio/tree/master/src/modules/ruxc).

### Example Of Usage ###

On MacOS:

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

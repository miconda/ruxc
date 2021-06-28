# ruxc #

`RUst eXports to C`

C library exporting useful functions written in Rust.

## Build ##

```
git clone https://github.com/miconda/ruxc
cd ruxc
cargo build --release
```

### Example ###

On MacOS:

```
gcc -o examples/httpget -I include/ examples/httpget.c target/release/libruxc.a -framework Security
./examples/httpget
```

## License ##

MIT

**Copyright**: `Daniel-Constantin Mierla <miconda@gmail.com>`

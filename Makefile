# Makefile to build the app and libs
#

include Makefile.defs

ifeq ($(OS),darwin)
LD_EXTRA_FLAGS= -framework Security
SOBASENAME=libruxc.dylib
else
ifeq ($(OS),linux)
LD_EXTRA_FLAGS= -lpthread -ldl
endif

SOBASENAME=libruxc.so
endif

ABASENAME=libruxc.a

.PHONY: all
all: lib example

.PHONY: lib
lib:
	cargo build --release

.PHONY: example
example:
	gcc -o examples/httpcli -I include/ examples/httpcli.c target/release/libruxc.a $(LD_EXTRA_FLAGS)

.PHONY: install-dirs
install-dirs:
	mkdir -p ${DESTDIR}${PREFIX}
	mkdir -p ${DESTDIR}${PREFIX}/bin
	mkdir -p ${DESTDIR}${PREFIX}/lib
	mkdir -p ${DESTDIR}${PREFIX}/lib/pkgconfig
	mkdir -p ${DESTDIR}${PREFIX}/include

.PHONY: install-lib
install-lib:
	cp include/ruxc.h ${DESTDIR}${PREFIX}/include/
	cp target/release/${ABASENAME} ${DESTDIR}${PREFIX}/lib/
	cp target/release/${SOBASENAME} ${DESTDIR}${PREFIX}/lib/
	cp misc/pkg-config/libruxc.pc ${DESTDIR}${PREFIX}/lib/pkgconfig/

.PHONY: install
install: install-dirs install-lib

.PHONY: uninstall
uninstall:
	rm -f ${DESTDIR}${PREFIX}/include/ruxc.h
	rm -f ${DESTDIR}${PREFIX}/lib/${ABASENAME}
	rm -f ${DESTDIR}${PREFIX}/lib/${SOBASENAME}
	rm -f ${DESTDIR}${PREFIX}/lib/pkgconfig//libruxc.pc

.PHONY: clean
clean:
	rm -rf target
	rm -f examples/httpcli


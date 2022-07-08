#!/bin/bash

# Uses cross, See: https://github.com/cross-rs/cross

CROSSBIN="$HOME/.cargo/bin/cross"
CROSSARGS="build --release"

# https://github.com/cross-rs/cross#supported-targets
ARCHS="x86_64-unknown-linux-gnu x86_64-unknown-freebsd x86_64-unknown-netbsd powerpc64-unknown-linux-gnu powerpc64le-unknown-linux-gnu aarch64-unknown-linux-gnu arm-unknown-linux-gnueabi"

for t in $ARCHS
do
  $CROSSBIN $CROSSARGS --target $t
done

for t in $ARCHS
do
  cp LICENSE target/$t/release
  cp README.md target/$t/release
done

mkdir release

for t in $ARCHS
do
  pushd "target/$t/release" || return
  tar --numeric-owner --owner=0 --group=0 -zcvf ../../../release/samanlainen-$t.tar.gz LICENSE README.md samanlainen
  popd || return
done

pushd release || return
sha256sum *.tar.gz > checksums.sha256
popd || return

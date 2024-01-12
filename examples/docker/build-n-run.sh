#!/bin/bash
set -ex
pushd ../../
cargo build --release
popd
mkdir -p workdir
cp ../../target/release/memoirctl ./workdir/
docker build \
    -t memoir/docker-example \
    --build-arg userid="$(id -u)" \
    --build-arg groupid="$(id -g)" \
    .
docker run \
    --rm \
    -v ./workdir/:/memoir/ \
    memoir/docker-example

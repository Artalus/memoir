#!/bin/bash
set -ex
./memoirctl run &
CONAN_HOME="$(pwd)/.conan2"
export CONAN_HOME
conan profile detect
conan install --build=\* . -c tools.cmake.cmaketoolchain:generator=Ninja
./memoirctl save memoir.csv
./memoirctl stop
wait

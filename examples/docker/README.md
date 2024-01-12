Using Memoir to profile build process
---

The main intention for `memoir` was to gather stats on how much memory is required
by C++ compilers during the CI build. This example replicates the process:

- `workdir/conanfile.py` is a simple stub for a C++ project using `Boost` as a "dependency".

- `entrypoint.sh` is used to run the build via `conan install --build`, launching
Memoir beforehand and then collecting a memory profile once build finishes.

- `Dockerfile` is used to provide build environment, installing packages required to
build `Boost` and its dependencies.

Run `build-n-run.sh` script to build the Memoir binaries, the Docker image (tagged
as `memoir/docker-example`) and the C++ "project". The resulting report will be put
under `workdir/memoir.csv`.

You can use the generated file as an input for the [`pyplot`](/examples/pyplot/) example.

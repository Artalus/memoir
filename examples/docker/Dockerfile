from debian:12-slim

run apt update \
    && apt install -y --no-install-recommends \
        gcc \
        g++ \
        cmake \
        ninja-build \
        python3-full \
        pipx

arg userid
arg groupid
run groupadd \
        --gid ${groupid} \
        memoir \
    && useradd \
        --uid ${userid} \
        --gid ${groupid} \
        --create-home \
        memoir
run mkdir /memoir \
    && chown memoir:memoir /memoir \
    && chmod 755 /memoir

user memoir
run pipx ensurepath && pipx install conan
workdir /memoir
copy ./entrypoint.sh /
user root
run apt install -y --no-install-recommends make
user memoir
cmd ["sh", "-lc", "/entrypoint.sh"]

FROM rust:1.45.2

RUN apt update && apt install -y apt-utils

WORKDIR /code

CMD cargo run -- --version v0.0.1-alpha.21 --message "Release new version" --github-token=asd

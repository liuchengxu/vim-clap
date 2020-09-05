#!/usr/bin/env bash

# Install Docker
sudo apt-get update
sudo apt-get install -y apt-transport-https ca-certificates curl gnupg-agent software-properties-common
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
sudo add-apt-repository "deb [arch=amd64] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable"
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io
docker --version

# Build the static binary image based on ekidd/rust-musl-builder
docker build -t build-maple-static-binary-image .
docker run --name build-maple-static-binary build-maple-static-binary-image

# Move the compiled binary to local fs.
mkdir -p target/release/
docker cp build-maple-static-binary:/home/rust/src/target/x86_64-unknown-linux-musl/release/maple ./target/release/maple
./target/release/maple version

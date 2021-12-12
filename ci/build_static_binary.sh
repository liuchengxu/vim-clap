#!/usr/bin/env bash

# Install Docker
sudo apt-get update
sudo apt-get install -y apt-transport-https ca-certificates curl gnupg-agent software-properties-common
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
sudo add-apt-repository "deb [arch=amd64] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable"
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io
docker --version

docker pull clux/muslrust
docker run -v $PWD:/volume --rm -t clux/muslrust cargo build --release --locked

mkdir -p target/release
sudo cp target/x86_64-unknown-linux-musl/release/maple target/release/maple

./target/release/maple version

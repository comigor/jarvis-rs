#!/bin/bash
apt install -y jq

wget https://go.dev/dl/go1.24.4.linux-amd64.tar.gz -O /tmp/go.tar.gz
rm -rf /usr/local/go && tar -C /usr/local -xzf /tmp/go.tar.gz
export PATH=$PATH:/usr/local/go/bin
go version

curl https://sh.rustup.rs -sSf | sh -s -- -y

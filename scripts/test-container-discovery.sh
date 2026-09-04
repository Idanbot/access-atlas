#!/usr/bin/env sh
set -eu

image=access-atlas:mock-connections-test

docker build \
  --file tests/container/Dockerfile \
  --tag "$image" \
  .
docker run --rm "$image"

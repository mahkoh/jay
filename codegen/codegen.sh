#!/bin/bash

set -ex

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

cargo run -p codegen

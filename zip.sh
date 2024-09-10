#!/usr/bin/env bash

set -e
mv green.zip temp || true
./build_wasm.sh
zip -r green.zip collections *.js why.sf2 system sounds index.html green_bg.wasm fs.json


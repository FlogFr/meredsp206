#!/bin/sh
# Render every design/src/*.html mockup to design/<name>.png with headless Chromium.
# Usage: design/render.sh            (run from anywhere)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
browser=${CHROMIUM:-chromium}
for src in "$here"/src/*.html; do
    name=$(basename "$src" .html)
    "$browser" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
        --window-size=1440,900 --virtual-time-budget=2000 \
        --screenshot="$here/$name.png" "file://$src" >/dev/null 2>&1
    echo "rendered $name.png"
done

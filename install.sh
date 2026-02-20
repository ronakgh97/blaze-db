#!/bin/bash
set -e

BASE_URL="https://downloads.blazedb.online"
OS="linux"
ARCH="x86_64"

FILE="blazedb-$OS-$ARCH"
URL="$BASE_URL/releases/$FILE"
CHECKSUM_URL="$URL.sha256"

TMP_DIR=$(mktemp -d)
cd "$TMP_DIR"

echo "Downloading blazedb..."

curl -fsSL "$URL" -o "$FILE"
curl -fsSL "$CHECKSUM_URL" -o "$FILE.sha256"

echo "Verifying checksum..."
if ! sha256sum -c "$FILE.sha256"; then
  echo "Checksum verification failed!"
  exit 1
fi

chmod +x "$FILE"
sudo mv "$FILE" /usr/local/bin/blazedb

echo "Installed successfully."
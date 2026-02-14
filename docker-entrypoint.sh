#!/bin/bash
set -e

# Create necessary directories with proper permissions
mkdir -p /home/blazedb/.config/blaze /home/blazedb/blaze/sources
chmod -R 755 /home/blazedb/blaze/sources
chmod -R 755 /home/blazedb/.config/blaze

# Initialize config if it doesn't exist
if [ ! -f "/home/blazedb/.config/blaze/SERVER_DATA.json" ]; then
    echo "Server file not found. Running initialization..."
    /app/blzsrv init
    echo "Initialization complete."
else
    echo "Server file found. Skipping initialization."
fi

# Ensure source directory exists
# shellcheck disable=SC2107
if [ ! -d "/home/blazedb/blaze/sources/default_src" && ! -d "/home/blazedb/blaze/backups" ]; then
    echo "Source directory not found. Creating default_src..."
    mkdir -p /home/blazedb/blaze/sources/default_src
    chmod -R 755 /home/blazedb/blaze/sources/default_src
    echo "Source directory created."
else
    echo "Source directory found."
fi

# Start the server
# shellcheck disable=SC2145
echo "Starting blzsrv with args: $@"
exec /app/blzsrv "$@"

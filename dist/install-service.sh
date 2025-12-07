#!/bin/bash
# Install rusty-sweeper-monitor as a systemd user service
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVICE_FILE="rusty-sweeper-monitor.service"
USER_SERVICE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

# Check if service file exists
if [ ! -f "$SCRIPT_DIR/$SERVICE_FILE" ]; then
    echo "Error: $SERVICE_FILE not found in $SCRIPT_DIR"
    exit 1
fi

# Create directory if needed
mkdir -p "$USER_SERVICE_DIR"

# Copy service file
cp "$SCRIPT_DIR/$SERVICE_FILE" "$USER_SERVICE_DIR/"
echo "Installed $SERVICE_FILE to $USER_SERVICE_DIR/"

# Reload systemd
systemctl --user daemon-reload
echo "Reloaded systemd user daemon"

echo ""
echo "Service installed successfully!"
echo ""
echo "To enable and start the service:"
echo "  systemctl --user enable rusty-sweeper-monitor"
echo "  systemctl --user start rusty-sweeper-monitor"
echo ""
echo "To check status:"
echo "  systemctl --user status rusty-sweeper-monitor"
echo ""
echo "To view logs:"
echo "  journalctl --user -u rusty-sweeper-monitor -f"

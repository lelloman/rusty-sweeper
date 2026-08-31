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

# Resolve the source-installed monitor (normally under ~/.cargo/bin) and
# specialize the packaged unit template for this user's installation.
MONITOR_BIN="$(command -v rusty-sweeper-monitor || true)"
if [ -z "$MONITOR_BIN" ]; then
    echo "Error: rusty-sweeper-monitor is not installed or not in PATH"
    exit 1
fi

sed "s|^ExecStart=.*|ExecStart=$MONITOR_BIN|" \
    "$SCRIPT_DIR/$SERVICE_FILE" > "$USER_SERVICE_DIR/$SERVICE_FILE"
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

#!/bin/sh
DIR="$(cd "$(dirname "$0")" && pwd)"
LABEL=com.projectswitch.hotkey
DOMAIN="gui/$(id -u)"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"

if [ -f "$PLIST" ]; then
    launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null
    launchctl bootstrap "$DOMAIN" "$PLIST" && exit 0
fi

env -i HOME="$HOME" USER="$USER" LOGNAME="$LOGNAME" SHELL="$SHELL" TMPDIR="$TMPDIR" PATH=/usr/bin:/bin:/usr/sbin:/sbin \
    nohup "$DIR/project-switch-hotkey" >/dev/null 2>&1 &

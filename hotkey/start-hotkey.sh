#!/bin/sh
DIR="$(cd "$(dirname "$0")" && pwd)"
nohup "$DIR/project-switch-hotkey" &>/dev/null &

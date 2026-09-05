#!/bin/sh
DIR="$(cd "$(dirname "$0")" && pwd)"

scrub_inherited_claude_session_env() {
    for var in $(env | sed -n 's/^\(CLAUDE[A-Za-z0-9_]*\)=.*/\1/p'); do
        unset "$var"
    done
}

scrub_inherited_claude_session_env

nohup "$DIR/project-switch-hotkey" >/dev/null 2>&1 &

#!/usr/bin/env bash
# PreToolUse hook: when Claude invokes `git commit`, inject the project's
# git conventions into the conversation so the commit follows them.
# The settings.json `if` matcher already filters to `git commit` invocations,
# so this script just emits the reminder.
#
# This is a mitigation for Opus 4.7 who is not good at following the instructions
# in CLAUDE.md / AGENTS.md, unlike the past models.

set -euo pipefail

conventions_file="dev/conventions/git.md"
if [ ! -f "$conventions_file" ]; then
  exit 2
fi

reminder="Before creating a Git commit, MUST read $conventions_file and follow it."

jq -n --arg ctx "$reminder" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    additionalContext: $ctx
  }
}'

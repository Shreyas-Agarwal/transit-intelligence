#!/usr/bin/env bash
# Pre-push secret scan: gitleaks protect only covers staged changes, so this
# is defense in depth against a commit that slipped past pre-commit (e.g. via
# `git commit --no-verify`) or was authored on a machine without hooks
# installed.
#
# Git passes push refs on stdin, one line per ref:
#   <local ref> <local sha1> <remote ref> <remote sha1>
# A remote sha1 of all zeros means the remote ref doesn't exist yet (new
# branch/tag) — in that case there's no remote tip to diff against, so this
# scans the whole local history of that ref instead.
set -euo pipefail

zero_sha="0000000000000000000000000000000000000000"
status=0

while read -r local_ref local_sha remote_ref remote_sha; do
  [ "$local_sha" = "$zero_sha" ] && continue # deleting a ref — nothing to scan

  if [ "$remote_sha" = "$zero_sha" ]; then
    # New branch/tag: no remote tip to diff against. Scan only commits not
    # already reachable from an existing remote-tracking ref, instead of
    # walking the entire history from $local_sha (which would rescan every
    # commit already on main/other branches on every new-branch push).
    range="$local_sha --not --remotes"
  else
    range="$remote_sha..$local_sha"
  fi

  echo "gitleaks: scanning $range ($local_ref)"
  gitleaks detect --source . --log-opts="$range" --redact || status=1
done

exit "$status"

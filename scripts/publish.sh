#!/usr/bin/env bash
set -Eeuo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd -- "$root"
repo='Dicklesworthstone/franken_alignment'
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
for command in git gh cargo; do
    command -v "$command" >/dev/null 2>&1 || fail "Required command is unavailable: $command"
done
[[ -f README.md && -f COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md && -f LICENSE ]] || fail 'Repository markers are missing.'
[[ ! -e .git ]] || fail 'A Git repository/worktree already exists here. This first-publication script will not change it.'
if git rev-parse --show-toplevel >/dev/null 2>&1; then
    fail 'This directory is inside another Git worktree. Extract the ZIP outside that worktree.'
fi
git config user.name >/dev/null || fail 'Configure your own Git user.name before publishing.'
git config user.email >/dev/null || fail 'Configure your own Git user.email before publishing.'
gh auth status --hostname github.com || fail 'Authenticate the GitHub CLI locally before publishing.'
login=$(gh api --hostname github.com user --jq .login) || fail 'Cannot verify the authenticated GitHub account.'
[[ "$login" == 'Dicklesworthstone' ]] || fail "Authenticated as $login, not Dicklesworthstone; refusing publication."
errors=$(mktemp)
trap 'rm -f -- "$errors"' EXIT
if gh api --hostname github.com "repos/$repo" >/dev/null 2>"$errors"; then
    fail "$repo already exists; refusing to modify it."
elif ! grep -q 'HTTP 404' "$errors"; then
    cat -- "$errors" >&2
    fail 'Remote existence check failed ambiguously; no repository will be created.'
fi
printf '%s\n' 'Checking the complete local file set before publication...'
cargo run --locked -p xtask -- check
printf '%s\n' "Creating NEW PUBLIC repository $repo from this directory."
git init -b main
git add --all
git commit -m 'docs: introduce FrankenAlignment design revision 0.2 and safe-Rust reference source'
if ! GH_HOST=github.com gh repo create "$repo" --public --source=. --remote=origin --push --description 'Design and executable reference for evidence-grounded AI monitoring, interpretation, and bounded control'; then
    fail 'Publication did not finish. Local Git history is preserved. The remote may have been created before a push failure; inspect both states rather than rerunning blindly.'
fi
printf '\nPublished: https://github.com/%s\n' "$repo"

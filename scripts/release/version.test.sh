#!/usr/bin/env bash
# Tests for version.sh against throwaway git repositories.
#
#   scripts/release/version.test.sh
set -euo pipefail

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/version.sh"
# Scratch space on real disk (RUNNER_TEMP in Actions), never a RAM-backed /tmp
scratch_root=${RUNNER_TEMP:-${XDG_CACHE_HOME:-$HOME/.cache}}
mkdir -p "$scratch_root"
work=$(mktemp -d "$scratch_root/version-test.XXXXXX")
trap 'rm -rf "$work"' EXIT

failures=0
n=0

# A fresh repository with Cargo.toml at `version`, then `cd` into it.
repo() {
  n=$((n + 1))
  local dir="$work/repo$n"
  mkdir -p "$dir"
  cd "$dir"
  git init -q -b master
  git config user.name "Test"
  git config user.email "test@example.invalid"
  git config commit.gpgsign false
  git config tag.gpgsign false
  printf '[package]\nname = "demo"\nversion = "%s"\n' "${1:-0.1.0}" >Cargo.toml
  git add Cargo.toml
  git commit -q -m "chore: start"
}

# commit SUBJECT [BODY]
commit() {
  echo "$RANDOM $1" >>log.txt
  git add log.txt
  if [[ -n ${2:-} ]]; then
    git commit -q -m "$1" -m "$2"
  else
    git commit -q -m "$1"
  fi
}

expect() {
  local name=$1 want=$2 got=$3
  if [[ $got == "$want" ]]; then
    echo "ok   $name"
  else
    echo "FAIL $name: want '$want', got '$got'"
    failures=$((failures + 1))
  fi
}

# No stable tag yet: the version in Cargo.toml
repo 0.4.2
commit "feat: something"
expect "no tag uses Cargo.toml" "0.4.2" "$("$script" next)"

# Only fixes since the tag: a patch
repo 1.0.0
git tag v1.2.3
commit "fix: one"
commit "docs: two"
expect "fixes bump the patch" "1.2.4" "$("$script" next)"
expect "last stable tag" "v1.2.3" "$("$script" last)"

# A feat anywhere since the tag (not only the newest commit): a minor
repo 1.0.0
git tag v1.2.3
commit "fix: first"
commit "feat(ui): a feature"
commit "fix: newest"
expect "an older feat bumps the minor" "1.3.0" "$("$script" next)"

# feat! anywhere since the tag: a major
repo 1.0.0
git tag v1.2.3
commit "feat!: drop the old API"
commit "feat: newer"
commit "fix: newest"
expect "an older feat! bumps the major" "2.0.0" "$("$script" next)"

# A scoped breaking marker
repo 1.0.0
git tag v1.2.3
commit "fix(api)!: rename a field"
commit "chore: newest"
expect "fix(scope)! bumps the major" "2.0.0" "$("$script" next)"

# BREAKING CHANGE in the body of an older commit: a major
repo 1.0.0
git tag v1.2.3
commit "feat: rework storage" "BREAKING CHANGE: the old files are no longer read"
commit "fix: newest"
expect "an older BREAKING CHANGE footer bumps the major" "2.0.0" "$("$script" next)"

# A BREAKING CHANGE footer after other body lines
repo 1.0.0
git tag v1.2.3
commit "refactor: split modules" "Some context.

BREAKING-CHANGE: the config moved"
commit "docs: newest"
expect "BREAKING-CHANGE after a body paragraph" "2.0.0" "$("$script" next)"

# While on 0.x a breaking change bumps the minor
repo 0.1.0
git tag v0.3.1
commit "feat!: breaking"
commit "fix: newest"
expect "0.x breaking bumps the minor" "0.4.0" "$("$script" next)"

# Only commits since the last stable tag count; pre-release tags are ignored
repo 1.0.0
commit "feat!: long ago"
git tag v1.2.3
commit "fix: after"
git tag v1.2.4-rc.1
commit "fix: newest"
expect "only commits since the last stable tag" "1.2.4" "$("$script" next)"
expect "stable tag on a commit" "v1.2.3" "$("$script" stable-at v1.2.3)"
expect "no stable tag on HEAD" "" "$("$script" stable-at HEAD)"

# Release notes sort commits into sections
repo 1.0.0
git tag v1.0.0
commit "feat(ui): a feature"
commit "fix: a fix"
commit "feat!: breaking" "BREAKING CHANGE: yes"
commit "chore(release): 1.1.0"
notes=$("$script" notes v1.0.0 HEAD)
expect "notes have a breaking section" "1" "$(grep -c '^### Breaking changes$' <<<"$notes")"
expect "notes list both features" "2" "$(sed -n '/^### Features$/,/^###/p' <<<"$notes" | grep -c '^- ')"
expect "notes keep the scope" "1" "$(grep -c '^- \*\*ui:\*\* a feature' <<<"$notes")"
expect "notes skip release commits" "0" "$(grep -c '1.1.0' <<<"$notes" || true)"

if ((failures > 0)); then
  echo "$failures failed"
  exit 1
fi
echo "all passed"

#!/usr/bin/env bash
# Versions from conventional commits. Git tags are the source of truth: the last stable
# `vX.Y.Z` tag reachable from a commit, bumped by the commits since (breaking change: major,
# feat: minor, anything else: patch; while on 0.x a breaking change bumps the minor). With no
# stable tag yet, the first release is the version in Cargo.toml.
#
#   version.sh next [REV]          next stable version for REV (default HEAD), e.g. 3.1.0
#   version.sh last [REV]          last stable tag reachable from REV, or nothing
#   version.sh stable-at REV       the stable tag on REV itself, or nothing
#   version.sh notes FROM TO       markdown release notes for FROM..TO (FROM may be empty)
set -euo pipefail

stable_re='^v[0-9]+\.[0-9]+\.[0-9]+$'

last_stable() {
  git tag --merged "${1:-HEAD}" --list 'v*' --sort=-v:refname | grep -E "$stable_re" | head -n1 || true
}

stable_at() {
  git tag --points-at "$1" --list 'v*' --sort=-v:refname | grep -E "$stable_re" | head -n1 || true
}

# Subjects and bodies of FROM..TO, one record per commit, NUL-separated.
commits() {
  local from=$1 to=$2
  if [[ -n $from ]]; then
    git log --no-merges --format='%s%n%b%x00' "$from..$to"
  else
    git log --no-merges --format='%s%n%b%x00' "$to"
  fi
}

next() {
  local rev=${1:-HEAD} tag base
  tag=$(last_stable "$rev")
  if [[ -z $tag ]]; then
    git show "$rev:Cargo.toml" | sed -n 's/^version = "\([0-9]*\.[0-9]*\.[0-9]*\)".*/\1/p' | head -n1
    return
  fi
  base=${tag#v}
  local bump=patch subject
  while IFS= read -r -d '' record; do
    # Every record after the first starts with the newline that followed the last NUL
    record=${record#$'\n'}
    [[ -z $record ]] && continue
    subject=${record%%$'\n'*}
    if [[ $subject =~ ^[a-z]+(\([^\)]*\))?!: || $record == *$'\n'BREAKING[\ -]CHANGE:* ]]; then
      bump=major
      break
    elif [[ $subject =~ ^feat(\([^\)]*\))?: ]]; then
      bump=minor
    fi
  done < <(commits "$tag" "$rev")
  local major minor patch
  IFS=. read -r major minor patch <<<"$base"
  if [[ $bump == major && $major == 0 ]]; then bump=minor; fi
  case $bump in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    patch) echo "$major.$minor.$((patch + 1))" ;;
  esac
}

notes() {
  local from=$1 to=$2 repo_url=${REPO_URL:-} record subject body sha
  local -a breaking=() features=() fixes=() other=()
  while IFS= read -r -d '' record; do
    record=${record#$'\n'}
    [[ -z $record ]] && continue
    sha=${record%%$'\n'*}
    record=${record#*$'\n'}
    subject=${record%%$'\n'*}
    body=${record#*$'\n'}
    [[ $subject =~ ^chore\(release\) ]] && continue
    local link=${sha:0:7}
    [[ -n $repo_url ]] && link="[${sha:0:7}]($repo_url/commit/$sha)"
    local text=$subject
    if [[ $subject =~ ^[a-z]+(\(([^\)]*)\))?!?:\ *(.*)$ ]]; then
      text=${BASH_REMATCH[3]}
      [[ -n ${BASH_REMATCH[2]} ]] && text="**${BASH_REMATCH[2]}:** $text"
    fi
    local line="- $text ($link)"
    if [[ $subject =~ ^[a-z]+(\([^\)]*\))?!: || $body == *BREAKING[\ -]CHANGE:* ]]; then
      breaking+=("$line")
    fi
    if [[ $subject =~ ^feat(\(|!|:) ]]; then
      features+=("$line")
    elif [[ $subject =~ ^(fix|perf)(\(|!|:) ]]; then
      fixes+=("$line")
    else
      other+=("$line")
    fi
  done < <(
    if [[ -n $from ]]; then range="$from..$to"; else range=$to; fi
    git log --no-merges --format='%H%n%s%n%b%x00' "$range"
  )
  section() {
    local title=$1
    shift
    (($# == 0)) && return
    printf '### %s\n\n' "$title"
    printf '%s\n' "$@"
    printf '\n'
  }
  section "Breaking changes" "${breaking[@]}"
  section "Features" "${features[@]}"
  section "Fixes" "${fixes[@]}"
  section "Other changes" "${other[@]}"
}

cmd=${1:-}
shift || true
case $cmd in
  next) next "${1:-HEAD}" ;;
  last) last_stable "${1:-HEAD}" ;;
  stable-at) stable_at "${1:?REV required}" ;;
  notes) notes "${1:-}" "${2:-HEAD}" ;;
  *)
    sed -n '2,11p' "$0" >&2
    exit 2
    ;;
esac

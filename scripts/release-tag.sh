#!/usr/bin/env bash

set -euo pipefail

fail() {
    echo "$1" >&2
    exit 1
}

[[ "$#" -eq 1 ]] \
    || fail "Usage: ${0##*/} YEAR.MONTH.RELEASE"

version="$1"
tag="v${version}"
pattern='^[1-9][0-9]{3}\.([1-9]|1[0-2])\.(0|[1-9][0-9]*)$'

[[ "${version}" =~ ${pattern} ]] \
    || fail "Version must match YEAR.MONTH.RELEASE, for example 2026.7.0."
[[ -z "$(git status --porcelain --untracked-files=all)" ]] \
    || fail "The worktree must be clean, including untracked files."

branch="$(git symbolic-ref --quiet --short HEAD)" \
    || fail "Releases must be created from the main branch."
[[ "${branch}" == "main" ]] \
    || fail "Releases must be created from main; current branch is ${branch}."

git fetch --prune --tags origin "refs/heads/main:refs/remotes/origin/main"
[[ "$(git rev-parse HEAD)" == "$(git rev-parse refs/remotes/origin/main)" ]] \
    || fail "Local HEAD must exactly match origin/main."
! git show-ref --verify --quiet "refs/tags/${tag}" \
    || fail "Tag ${tag} already exists."

printf 'Create and push %s from %s? Type yes to continue: ' \
    "${tag}" "$(git rev-parse --short=12 HEAD)"
confirmation=""
IFS= read -r confirmation || true
[[ "${confirmation}" == "yes" ]] || {
    echo "Release cancelled; no tag was created."
    exit 0
}

git tag --annotate "${tag}" --message "Release ${version}"
git push origin "refs/tags/${tag}:refs/tags/${tag}"
echo "Published tag: ghcr.io/karanbalani/retsu:${version}"
echo "The calendar-version tag starts the container release workflow; no latest tag is published."

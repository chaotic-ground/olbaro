#!/usr/bin/env bash
# Splits Harper's browser extension packages out of Automattic/harper and
# merges them into a branch based on the current HEAD.
#
# The split is deterministic: the same upstream commit always produces the
# same rewritten history, so each run's split shares its history with the
# previous one and merges as an ordinary update.
#
# Writes `status` (up-to-date, merged or conflict), `upstream_sha` and
# `split_sha` to $GITHUB_OUTPUT when it is set.
set -euo pipefail

UPSTREAM_URL="${UPSTREAM_URL:-https://github.com/Automattic/harper.git}"
UPSTREAM_REF="${UPSTREAM_REF:-master}"
SYNC_BRANCH="${SYNC_BRANCH:-sync/harper}"
PATHS=(
	packages/chrome-plugin
	packages/lint-framework
	packages/components
)

output() {
	echo "$1=$2"
	if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
		echo "$1=$2" >>"$GITHUB_OUTPUT"
	fi
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

git clone --quiet --single-branch --branch "$UPSTREAM_REF" "$UPSTREAM_URL" "$work/harper"
upstream_sha="$(git -C "$work/harper" rev-parse HEAD)"

filter_args=()
for path in "${PATHS[@]}"; do
	filter_args+=(--path "$path")
done
git -C "$work/harper" filter-repo --force --quiet "${filter_args[@]}"
split_sha="$(git -C "$work/harper" rev-parse HEAD)"

output upstream_sha "$upstream_sha"
output split_sha "$split_sha"

git fetch --quiet "$work/harper" HEAD

if git merge-base --is-ancestor "$split_sha" HEAD 2>/dev/null; then
	output status up-to-date
	exit 0
fi

git switch --quiet -C "$SYNC_BRANCH"
message="Sync Harper's extension packages to ${upstream_sha:0:8}

Split from ${UPSTREAM_URL%.git}/commit/$upstream_sha
with git filter-repo $(printf -- '--path %s ' "${PATHS[@]}")"

if git merge --quiet --no-ff --allow-unrelated-histories -m "$message" "$split_sha"; then
	output status merged
else
	git diff --name-only --diff-filter=U | sed 's/^/conflict: /'
	git merge --abort
	output status conflict
fi

#!/usr/bin/env bash
# Splits Harper's browser extension packages out of Automattic/harper and
# merges them into a branch based on the current HEAD.
#
# The last synced upstream commit is recorded in $STATE_FILE rather than in
# git history, because this repository squashes pull requests and so the
# split history never becomes an ancestor of the default branch. The split is
# deterministic: the same upstream commit always produces the same rewritten
# history, so re-splitting the recorded commit gives the base for a three-way
# merge of upstream's changes into ours.
#
# Writes `status` (up-to-date, merged or conflict), `upstream_sha` and
# `split_sha` to $GITHUB_OUTPUT when it is set.
set -euo pipefail

UPSTREAM_URL="${UPSTREAM_URL:-https://github.com/Automattic/harper.git}"
UPSTREAM_REF="${UPSTREAM_REF:-master}"
SYNC_BRANCH="${SYNC_BRANCH:-sync/harper}"
STATE_FILE="${STATE_FILE:-.github/harper-upstream}"
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

synced_sha=""
if [[ -f "$STATE_FILE" ]]; then
	synced_sha="$(tr -d '[:space:]' <"$STATE_FILE")"
fi

git clone --quiet --single-branch --branch "$UPSTREAM_REF" "$UPSTREAM_URL" "$work/harper"
upstream_sha="$(git -C "$work/harper" rev-parse HEAD)"
output upstream_sha "$upstream_sha"

if [[ "$synced_sha" == "$upstream_sha" ]]; then
	output status up-to-date
	exit 0
fi

# filter-repo rewrites every ref, so this one ends up on the split commit
# that corresponds to the last synced upstream commit.
if [[ -n "$synced_sha" ]]; then
	git -C "$work/harper" branch synced "$synced_sha"
fi

filter_args=()
for path in "${PATHS[@]}"; do
	filter_args+=(--path "$path")
done
git -C "$work/harper" filter-repo --force --quiet "${filter_args[@]}"
split_sha="$(git -C "$work/harper" rev-parse HEAD)"
output split_sha "$split_sha"

if [[ -n "$synced_sha" ]]; then
	synced_split_sha="$(git -C "$work/harper" rev-parse synced)"
	# Upstream moved but none of the split paths changed.
	if [[ "$synced_split_sha" == "$split_sha" ]]; then
		output status up-to-date
		exit 0
	fi
fi

git fetch --quiet "$work/harper" HEAD
git switch --quiet -C "$SYNC_BRANCH"

if [[ -n "$synced_sha" ]]; then
	if ! tree="$(git merge-tree --write-tree --name-only --merge-base="$synced_split_sha" HEAD "$split_sha")"; then
		# The first line is the tree; the rest list the conflicted files.
		sed -e '1d' -e '/^$/,$d' -e 's/^/conflict: /' <<<"$tree"
		output status conflict
		exit 0
	fi
	git read-tree --reset -u "$tree"
else
	if ! git merge --quiet --no-commit --no-ff --allow-unrelated-histories "$split_sha"; then
		git diff --name-only --diff-filter=U | sed 's/^/conflict: /'
		git merge --abort
		output status conflict
		exit 0
	fi
fi

echo "$upstream_sha" >"$STATE_FILE"
git add "$STATE_FILE"
git commit --quiet -m "Sync Harper's extension packages to ${upstream_sha:0:8}

Split from ${UPSTREAM_URL%.git}/commit/$upstream_sha
with git filter-repo $(printf -- '--path %s ' "${PATHS[@]}")"
output status merged

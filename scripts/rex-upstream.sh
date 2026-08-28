#!/usr/bin/env sh
# The layout engine in crates/core is vendored from KenyC/ReX, not a dependency,
# so nothing bumps it for us. This lists what upstream has done since the vendored
# revision (crates/core/REX-UPSTREAM) so that porting fixes stays a deliberate,
# visible act. Update REX-UPSTREAM when you port up to a revision.
set -eu
cd "$(dirname "$0")/.."
base="$(tr -d '[:space:]' < crates/core/REX-UPSTREAM)"
echo "vendored KenyC/ReX @ $base"
gh api "repos/KenyC/ReX/compare/$base...main" \
  --jq '"upstream main is \(.ahead_by) commit(s) ahead", (.commits[] | "  \(.sha[0:7])  \(.commit.author.date[0:10])  \(.commit.message | split("\n")[0])")'

#!/usr/bin/env bash
# Invitations end to end: invite, preview, wrong-email rejection, invited sign-up, reuse, removal.
#   scripts/e2e-team.sh <owner email> <owner password> [base_url]
set -euo pipefail
EMAIL="$1"; PASS="$2"; BASE="${3:-http://localhost:3000}"
OWNER="$(mktemp)"; NEW="$(mktemp)"; trap 'rm -f "$OWNER" "$NEW"' EXIT
H=(-H "content-type: application/json" -H "x-seoseo-csrf: 1")
json() { python -c "import sys,json;d=json.load(sys.stdin);print(eval('d'+sys.argv[1]))" "$1"; }
pass() { printf '  ok   %s\n' "$1"; }
fail() { printf '  FAIL %s\n' "$1"; exit 1; }
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

curl -fsS -c "$OWNER" "${H[@]}" -X POST "$BASE/api/v1/auth/sign-in" -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" >/dev/null
INVITEE="mate$RANDOM@example.com"
inv=$(curl -fsS -b "$OWNER" "${H[@]}" -X POST "$BASE/api/v1/team/invitations" -d "{\"email\":\"$INVITEE\"}")
TOKEN=$(echo "$inv" | json "['token']"); IID=$(echo "$inv" | json "['invitation']['id']")
pass "owner creates invitation"
[[ $(code -b "$OWNER" "${H[@]}" -X POST "$BASE/api/v1/team/invitations" -d "{\"email\":\"$INVITEE\"}") == 409 ]] && pass "duplicate open invite rejected (409)"
org=$(curl -fsS "$BASE/api/v1/invitations/$TOKEN" | json "['organization_name']")
[[ -n $org ]] && pass "public preview shows workspace ($org)"
[[ $(code "$BASE/api/v1/invitations/not-a-real-token") == 404 ]] && pass "unknown token 404"
[[ $(code "${H[@]}" -X POST "$BASE/api/v1/auth/sign-up" -d "{\"email\":\"other@example.com\",\"name\":\"X\",\"password\":\"correct horse battery\",\"invite_token\":\"$TOKEN\"}") == 403 ]] \
  && pass "sign-up with a different email rejected (403)" || fail "wrong email accepted"
[[ $(code -c "$NEW" "${H[@]}" -X POST "$BASE/api/v1/auth/sign-up" -d "{\"email\":\"$INVITEE\",\"name\":\"Mate\",\"password\":\"correct horse battery\",\"invite_token\":\"$TOKEN\"}") == 201 ]] \
  && pass "invited sign-up works while sign-up is closed (201)" || fail "invited sign-up failed"
me=$(curl -fsS -b "$NEW" "$BASE/api/v1/me")
[[ $(echo "$me" | json "['role']") == admin && $(echo "$me" | json ".get('memberships').__len__()") == 1 ]] \
  && pass "invitee joined as admin, no personal workspace" || fail "membership wrong: $me"
OWNER_ORG=$(curl -fsS -b "$OWNER" "$BASE/api/v1/me" | json "['organization_id']")
[[ $(echo "$me" | json "['organization_id']") == "$OWNER_ORG" ]] && pass "invitee is in the owner's workspace"
[[ $(code "${H[@]}" -X POST "$BASE/api/v1/auth/sign-up" -d "{\"email\":\"$INVITEE\",\"name\":\"Again\",\"password\":\"correct horse battery\",\"invite_token\":\"$TOKEN\"}") == 404 ]] \
  && pass "invitation cannot be reused (404)"
n=$(curl -fsS -b "$NEW" "$BASE/api/v1/projects" | json ".__len__()")
[[ $n -ge 1 ]] && pass "invitee sees the workspace's projects ($n)"
UID_NEW=$(echo "$me" | json "['user']['id']")
OWNER_ID=$(curl -fsS -b "$OWNER" "$BASE/api/v1/me" | json "['user']['id']")
[[ $(code -b "$NEW" "${H[@]}" -X DELETE "$BASE/api/v1/team/members/$OWNER_ID") == 409 ]] && pass "owner cannot be removed (409)"
[[ $(code -b "$OWNER" "${H[@]}" -X DELETE "$BASE/api/v1/team/members/$UID_NEW") == 204 ]] && pass "owner removes member (204)"
[[ $(code -b "$NEW" "$BASE/api/v1/projects") == 401 ]] && pass "removed member loses access (401)"
inv2=$(curl -fsS -b "$OWNER" "${H[@]}" -X POST "$BASE/api/v1/team/invitations" -d '{"email":"revoke-me@example.com"}')
T2=$(echo "$inv2" | json "['token']"); I2=$(echo "$inv2" | json "['invitation']['id']")
[[ $(code -b "$OWNER" "${H[@]}" -X DELETE "$BASE/api/v1/team/invitations/$I2") == 204 ]] && pass "revoke invitation"
[[ $(code "$BASE/api/v1/invitations/$T2") == 404 ]] && pass "revoked invitation is dead (404)"
echo "e2e-team: all checks passed"

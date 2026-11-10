#!/usr/bin/env bash
# End-to-end smoke test against a running stack (through the web app's /api proxy).
# Usage: scripts/smoke.sh [base_url]   (default http://localhost:3000)
# Needs a fresh instance (no accounts yet) for the sign-up step.
set -euo pipefail
BASE="${1:-http://localhost:3000}"
JAR="$(mktemp)"
trap 'rm -f "$JAR"' EXIT
H=(-H "content-type: application/json" -H "x-seoseo-csrf: 1")
pass() { printf '  ok   %s\n' "$1"; }
fail() { printf '  FAIL %s\n' "$1"; exit 1; }
json() { python -c "import sys,json;d=json.load(sys.stdin);print(eval('d'+sys.argv[1]))" "$1"; }

echo "smoke: $BASE"
curl -fsS "$BASE/api/v1/info" | grep -q '"auth_mode"' && pass "GET /info via web proxy"
status=$(curl -fsS "$BASE/api/v1/auth/status")
[[ $(echo "$status" | json "['needs_setup']") == "True" ]] && pass "fresh instance needs setup" || fail "instance already has accounts ($status)"

EMAIL="admin@example.com"
code=$(curl -s -o /dev/null -w '%{http_code}' -c "$JAR" "${H[@]}" -X POST "$BASE/api/v1/auth/sign-up" \
  -d "{\"email\":\"$EMAIL\",\"name\":\"Admin\",\"password\":\"correct horse battery\"}")
[[ $code == 201 ]] && pass "sign-up first admin (201)" || fail "sign-up returned $code"

code=$(curl -s -o /dev/null -w '%{http_code}' "${H[@]}" -X POST "$BASE/api/v1/auth/sign-up" \
  -d '{"email":"second@example.com","name":"Second","password":"correct horse battery"}')
[[ $code == 403 ]] && pass "second sign-up is closed (403)" || fail "second sign-up returned $code"

me=$(curl -fsS -b "$JAR" "$BASE/api/v1/me")
[[ $(echo "$me" | json "['role']") == "owner" ]] && pass "GET /me role=owner"
[[ $(echo "$me" | json "['user']['is_instance_admin']") == "True" ]] && pass "first user is instance admin"

code=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" -H "content-type: application/json" \
  -X POST "$BASE/api/v1/projects" -d '{"name":"No CSRF"}')
[[ $code == 403 ]] && pass "cookie write without CSRF header rejected (403)" || fail "csrf check returned $code"

proj=$(curl -fsS -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/projects" \
  -d '{"name":"Example","domain":"https://Example.com/blog","country_code":"gb","language_code":"en"}')
PID=$(echo "$proj" | json "['id']")
[[ $(echo "$proj" | json "['domain']") == "example.com" ]] && pass "create project (domain normalized)"
[[ $(echo "$proj" | json "['country_code']") == "GB" ]] && pass "country normalized to GB"

code=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/projects" -d '{"name":"x","domain":"not a domain"}')
[[ $code == 422 ]] && pass "invalid domain rejected (422)" || fail "invalid domain returned $code"

upd=$(curl -fsS -b "$JAR" "${H[@]}" -X PATCH "$BASE/api/v1/projects/$PID" -d '{"brand_name":"Example Co","domain":null}')
[[ $(echo "$upd" | json "['brand_name']") == "Example Co" && $(echo "$upd" | json "['domain']") == "None" ]] && pass "patch sets brand, clears domain"

code=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "${H[@]}" -X DELETE "$BASE/api/v1/projects/$PID")
[[ $code == 409 ]] && pass "delete active project refused (409)" || fail "delete active returned $code"
curl -fsS -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/projects/$PID/archive" >/dev/null && pass "archive project"

key=$(curl -fsS -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/api-keys" -d '{"name":"smoke"}')
SECRET=$(echo "$key" | json "['secret']")
KID=$(echo "$key" | json "['key']['id']")
n=$(curl -fsS -H "authorization: Bearer $SECRET" "$BASE/api/v1/projects?archived=true" | json ".__len__()")
[[ $n == 1 ]] && pass "API key lists archived project (bearer, no CSRF needed)"

job=$(curl -fsS -H "authorization: Bearer $SECRET" -X POST "$BASE/api/v1/jobs/ping")
JID=$(echo "$job" | json "['id']")
for _ in $(seq 1 30); do
  st=$(curl -fsS -H "authorization: Bearer $SECRET" "$BASE/api/v1/jobs/$JID" | json "['status']")
  [[ $st == completed ]] && break
  sleep 1
done
[[ $st == completed ]] && pass "worker completed noop job" || fail "job status $st"

curl -fsS -b "$JAR" "${H[@]}" -X DELETE "$BASE/api/v1/api-keys/$KID" -o /dev/null && pass "revoke API key"
code=$(curl -s -o /dev/null -w '%{http_code}' -H "authorization: Bearer $SECRET" "$BASE/api/v1/projects")
[[ $code == 401 ]] && pass "revoked key rejected (401)" || fail "revoked key returned $code"

curl -fsS -b "$JAR" -c "$JAR" "${H[@]}" -X POST "$BASE/api/v1/auth/sign-out" -o /dev/null && pass "sign-out"
code=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "$BASE/api/v1/me")
[[ $code == 401 ]] && pass "session ended (401)" || fail "after sign-out /me returned $code"

code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/projects")
[[ $code == 307 ]] && pass "app page redirects to sign-in when signed out (307)" || fail "/projects returned $code"
code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/docs")
[[ $code == 200 ]] && pass "API docs served at /api/docs"
echo "smoke: all checks passed"

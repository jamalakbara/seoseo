#!/usr/bin/env bash
# Audit the badseo fixture end to end and check the expected issues are found.
# Needs the test overlay running and an existing account:
#   scripts/e2e-audit.sh <email> <password> [base_url]
set -euo pipefail
EMAIL="$1"; PASS="$2"; BASE="${3:-http://localhost:3000}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
H=(-H "content-type: application/json" -H "x-seoseo-csrf: 1")
json() { python -c "import sys,json;d=json.load(sys.stdin);print(eval('d'+sys.argv[1]))" "$1"; }

curl -fsS -c "$JAR" "${H[@]}" -X POST "$BASE/api/v1/auth/sign-in" -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" >/dev/null
PID=$(curl -fsS -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/projects" -d '{"name":"badseo e2e","domain":"badseo.test"}' | json "['id']")
AID=$(curl -fsS -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/projects/$PID/audits" \
  -d '{"start_url":"http://badseo/","max_pages":100,"delay_ms":0,"concurrency":4}' | json "['id']")
echo "audit $AID started"
for _ in $(seq 1 120); do
  A=$(curl -fsS -b "$JAR" "$BASE/api/v1/projects/$PID/audits/$AID")
  st=$(echo "$A" | json "['status']")
  [[ $st == completed || $st == failed ]] && break
  sleep 1
done
echo "$A" | python -c "import sys,json;a=json.load(sys.stdin);print('status',a['status'],'pages',a['pages_crawled'],'score',a['health_score'],'error',a['error'])"
[[ $st == completed ]] || { echo "FAIL audit did not complete"; exit 1; }

found=$(curl -fsS -b "$JAR" "$BASE/api/v1/projects/$PID/audits/$AID/issues/summary" | python -c "import sys,json;print(' '.join(sorted(i['issue_type'] for i in json.load(sys.stdin))))")
echo "found: $found"
missing=0
for t in missing-title duplicate-title duplicate-meta-description duplicate-content thin-content broken-page \
         broken-internal-link server-error redirect-chain redirect-loop blocked-page noindex-page canonicalized-page \
         canonical-conflict heading-order-skip multiple-h1 images-missing-alt title-too-long meta-description-too-short \
         invalid-structured-data javascript-rendering-suspected orphan-page deep-page robots-disallowed \
         redirected-internal-link sitemap-non-indexable soft-404 url-hygiene invalid-hreflang hreflang-missing-self hreflang-missing-return hreflang-target-error broken-asset; do
  if [[ " $found " == *" $t "* ]]; then printf '  ok   %s\n' "$t"; else printf '  MISS %s\n' "$t"; missing=1; fi
done
# Clean pages must not be flagged.
about=$(curl -fsS -b "$JAR" "$BASE/api/v1/projects/$PID/audits/$AID/pages?search=about.html" | json "['items'][0]['issue_count']")
[[ $about == 0 ]] && echo "  ok   about.html has no issues" || { echo "  FAIL about.html has $about issues"; missing=1; }
code=$(curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "$BASE/api/v1/projects/$PID/audits/$AID/issues.csv")
[[ $code == 200 ]] && echo "  ok   CSV export" || { echo "  FAIL CSV export $code"; missing=1; }
[[ $missing == 0 ]] && echo "e2e-audit: all checks passed" || { echo "e2e-audit: FAILED"; exit 1; }

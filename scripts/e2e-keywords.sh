#!/usr/bin/env bash
# Keyword research end to end against live free providers (needs internet):
#   scripts/e2e-keywords.sh <email> <password> [base_url]
# Checks: provider catalog + settings, key encryption guard, a seed expands to >= 100 ideas
# with popularity/intent, filters, CSV, saved keywords, and (when available) authority + SERP.
# Set E2E_AUTHORITY=1 to also import 20k Common Crawl domains and check SR lookups.
set -euo pipefail
EMAIL="$1"; PASS="$2"; BASE="${3:-http://localhost:3000}"
JAR="$(mktemp)"; trap 'rm -f "$JAR"' EXIT
H=(-H "content-type: application/json" -H "x-seoseo-csrf: 1")
json() { python -c "import sys,json;d=json.load(sys.stdin);print(eval('d'+sys.argv[1]))" "$1"; }
failed=0
ok()   { printf '  ok   %s\n' "$1"; }
bad()  { printf '  FAIL %s\n' "$1"; failed=1; }
check() { if [[ "$1" == "$2" ]]; then ok "$3"; else bad "$3 (got '$1', want '$2')"; fi; }
api()  { curl -fsS -b "$JAR" "${H[@]}" "$@"; }
code() { curl -s -o /dev/null -w '%{http_code}' -b "$JAR" "${H[@]}" "$@"; }

curl -fsS -c "$JAR" "${H[@]}" -X POST "$BASE/api/v1/auth/sign-in" -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" >/dev/null

# ---- providers
P=$(api "$BASE/api/v1/providers")
check "$(echo "$P" | json "[0]['id']")" "suggest_google" "provider catalog listed"
check "$(echo "$P" | python -c "import sys,json;print(all(p['ready'] for p in json.load(sys.stdin) if p['kind'] in ('suggest','demand')))")" "True" "free idea providers ready with zero config"
check "$(echo "$P" | python -c "import sys,json;print(any('secret' in k or k=='api_key' for p in json.load(sys.stdin) for k in p))")" "False" "no secrets in provider list"
check "$(code -X PUT "$BASE/api/v1/providers/nope" -d '{"enabled":true}')" "404" "unknown provider is 404"
check "$(code -X PUT "$BASE/api/v1/providers/suggest_google" -d '{"api_key":"x"}')" "422" "key on keyless provider rejected"
T=$(api -X POST "$BASE/api/v1/providers/suggest_google/test")
check "$(echo "$T" | json "['ok']")" "True" "live test of Google autocomplete ($(echo "$T" | json "['message']"))"
api -X PUT "$BASE/api/v1/providers/suggest_bing" -d '{"enabled":false}' >/dev/null
check "$(api "$BASE/api/v1/providers" | python -c "import sys,json;print([p['enabled'] for p in json.load(sys.stdin) if p['id']=='suggest_bing'][0])")" "False" "provider can be switched off"
api -X PUT "$BASE/api/v1/providers/suggest_bing" -d '{"enabled":true}' >/dev/null
kc=$(code -X PUT "$BASE/api/v1/providers/serp_brave" -d '{"api_key":"test-key-123"}')
if [[ $kc == 200 ]]; then
  check "$(api "$BASE/api/v1/providers" | python -c "import sys,json;print([p['has_key'] for p in json.load(sys.stdin) if p['id']=='serp_brave'][0])")" "True" "API key stored encrypted (has_key)"
  api -X PUT "$BASE/api/v1/providers/serp_brave" -d '{"api_key":""}' >/dev/null
  check "$(api "$BASE/api/v1/providers" | python -c "import sys,json;print([p['has_key'] for p in json.load(sys.stdin) if p['id']=='serp_brave'][0])")" "False" "API key removed"
else
  check "$kc" "409" "storing a key without APP_ENCRYPTION_KEY is refused"
fi

# ---- optional: domain authority import (small)
if [[ "${E2E_AUTHORITY:-0}" == "1" ]]; then
  api -X POST "$BASE/api/v1/authority/import" -d '{"limit":20000}' >/dev/null
  for _ in $(seq 1 90); do
    A=$(api "$BASE/api/v1/authority")
    [[ $(echo "$A" | json "['active_job']") == "None" ]] && break
    sleep 2
  done
  check "$(echo "$A" | python -c "import sys,json;s=json.load(sys.stdin)['status'];print(bool(s) and s['imported']>=19000)")" "True" "Common Crawl ranks imported"
  L=$(api "$BASE/api/v1/authority/lookup?domain=https://www.wikipedia.org/wiki/SEO")
  check "$(echo "$L" | python -c "import sys,json;d=json.load(sys.stdin);print(d['domain'], d['sr'] is not None and d['sr']>=70)")" "wikipedia.org True" "SR lookup normalizes to the registrable domain"
  check "$(api "$BASE/api/v1/authority/lookup?domain=no-such-domain-zz9.example" | json "['sr']")" "None" "unknown domain has null SR, not 0"
fi

# ---- research
PID=$(api -X POST "$BASE/api/v1/projects" -d '{"name":"keywords e2e","domain":"example.com","brand_name":"Acme"}' | json "['id']")
check "$(code -X POST "$BASE/api/v1/projects/$PID/keyword-research" -d '{"seed":"  "}')" "422" "empty seed rejected"
check "$(code -X POST "$BASE/api/v1/projects/$PID/keyword-research" -d '{"seed":"x","max_ideas":5000}')" "422" "max_ideas bounded"
RID=$(api -X POST "$BASE/api/v1/projects/$PID/keyword-research" -d '{"seed":"Standing  Desk","max_ideas":150,"serp_top":3}' | json "['id']")
echo "research $RID started"
for _ in $(seq 1 180); do
  R=$(api "$BASE/api/v1/projects/$PID/keyword-research/$RID")
  st=$(echo "$R" | json "['status']")
  [[ $st == completed || $st == failed ]] && break
  sleep 1
done
echo "$R" | python -c "import sys,json;r=json.load(sys.stdin);print('status',r['status'],'ideas',r['ideas_total'],'error',r['error']);[print('  warning:',w) for w in r['warnings']]"
check "$st" "completed" "research completed"
check "$(echo "$R" | json "['seed']")" "standing desk" "seed normalized"
check "$(echo "$R" | python -c "import sys,json;print(json.load(sys.stdin)['ideas_total']>=100)")" "True" "at least 100 ideas"

I=$(api "$BASE/api/v1/projects/$PID/keyword-research/$RID/ideas?limit=1000")
echo "$I" | python -c "
import sys,json
d=json.load(sys.stdin)['items']
print('  sample:', ', '.join(i['keyword'] for i in d[:6]))
print('  with volume:', sum(i['est_volume'] is not None for i in d), '| with kd:', sum(i['kd'] is not None for i in d), '| with serp features:', sum(i['serp_features'] is not None for i in d))"
check "$(echo "$I" | json "['items'][0]['keyword']")" "standing desk" "seed is the top idea"
check "$(echo "$I" | python -c "import sys,json;d=json.load(sys.stdin)['items'];print(all(1<=i['popularity']<=100 and i['intent'] in ('informational','commercial','transactional','navigational') for i in d))")" "True" "every idea has popularity and intent"
check "$(echo "$I" | python -c "import sys,json;d=json.load(sys.stdin)['items'];print(all((i['est_volume'] is None)==(i['volume_confidence'] is None)==(i['volume_bucket'] is None) and i['est_volume']!=0 for i in d))")" "True" "volume is null or labelled with source confidence, never 0"
check "$(echo "$I" | python -c "import sys,json;d=json.load(sys.stdin)['items'];p=[i['popularity'] for i in d];print(p==sorted(p,reverse=True))")" "True" "default sort is popularity desc"
check "$(echo "$I" | python -c "import sys,json;d=json.load(sys.stdin)['items'];print(len({i['keyword'] for i in d})==len(d))")" "True" "no duplicate keywords"
Q=$(api "$BASE/api/v1/projects/$PID/keyword-research/$RID/ideas?questions=true&limit=200")
check "$(echo "$Q" | python -c "import sys,json;d=json.load(sys.stdin);print(d['total']>0 and all('question' in i['sources'] for i in d['items']))")" "True" "question filter"
F=$(api "$BASE/api/v1/projects/$PID/keyword-research/$RID/ideas?search=conv%25erter&limit=10")
check "$(echo "$F" | json "['total']")" "0" "search treats % literally"
F=$(api "$BASE/api/v1/projects/$PID/keyword-research/$RID/ideas?intent=commercial&sort=keyword&limit=200")
check "$(echo "$F" | python -c "import sys,json;d=json.load(sys.stdin)['items'];k=[i['keyword'] for i in d];print(len(d)>0 and k==sorted(k) and all(i['intent']=='commercial' for i in d))")" "True" "intent filter + keyword sort"
check "$(code "$BASE/api/v1/projects/$PID/keyword-research/$RID/ideas.csv")" "200" "ideas CSV export"

# ---- saved keywords
IDS=$(echo "$I" | python -c "import sys,json;print(json.dumps([i['id'] for i in json.load(sys.stdin)['items'][:5]]))")
S=$(api -X POST "$BASE/api/v1/projects/$PID/saved-keywords" -d "{\"research_id\":\"$RID\",\"idea_ids\":$IDS,\"tags\":[\"Launch\"]}")
check "$(echo "$S" | json "['saved']")" "5" "save 5 ideas"
S=$(api -X POST "$BASE/api/v1/projects/$PID/saved-keywords" -d "{\"research_id\":\"$RID\",\"idea_ids\":$IDS}")
check "$(echo "$S" | json "['saved']") $(echo "$S" | json "['already_saved']")" "0 5" "saving again is idempotent"
S=$(api -X POST "$BASE/api/v1/projects/$PID/saved-keywords" -d '{"keywords":["Buy  Acme Desk","how to assemble a desk"]}')
check "$(echo "$S" | json "['saved']")" "2" "save manual keywords"
SV=$(api "$BASE/api/v1/projects/$PID/saved-keywords?tag=launch")
check "$(echo "$SV" | json "['total']")" "5" "tag filter (tags are normalized)"
SV=$(api "$BASE/api/v1/projects/$PID/saved-keywords?search=acme")
check "$(echo "$SV" | python -c "import sys,json;i=json.load(sys.stdin)['items'][0];print(i['keyword'], i['popularity'], i['kd'])")" "buy acme desk None None" "manual keyword has null metrics"
SID=$(echo "$SV" | json "['items'][0]['id']")
check "$(api -X PATCH "$BASE/api/v1/projects/$PID/saved-keywords/$SID" -d '{"tags":["b","a","a"]}' | python -c "import sys,json;print(json.load(sys.stdin)['tags'])")" "['a', 'b']" "retag (deduplicated)"
check "$(api "$BASE/api/v1/projects/$PID/saved-keywords/tags" | python -c "import sys,json;print(json.load(sys.stdin))")" "['a', 'b', 'launch']" "tag list"
check "$(code "$BASE/api/v1/projects/$PID/saved-keywords.csv")" "200" "saved CSV export"
check "$(api -X POST "$BASE/api/v1/projects/$PID/saved-keywords/delete" -d "{\"ids\":[\"$SID\"]}" | json "['deleted']")" "1" "delete saved keyword"

# ---- SERP analysis (needs a SERP provider; reported, not required)
sc=$(curl -s -o "$JAR.serp" -w '%{http_code}' -b "$JAR" "${H[@]}" -X POST "$BASE/api/v1/projects/$PID/serp-analysis" -d '{"keyword":"standing desk"}')
case $sc in
  200) python -c "
import sys,json
a=json.load(open(sys.argv[1]))
print('  ok   SERP analysis via', a['provider'], '| results', len(a['items']), '| kd', a['kd'], '| intent', a['intent'], '| cached', a['cached'], '| features', a['features'])" "$JAR.serp" ;;
  409) ok "SERP analysis explains missing provider: $(json "['detail']" < "$JAR.serp")" ;;
  503) echo "  note SERP provider unavailable right now: $(json "['detail']" < "$JAR.serp")" ;;
  *) bad "serp-analysis returned $sc" ;;
esac
rm -f "$JAR.serp"

# ---- tenancy + cleanup
check "$(code "$BASE/api/v1/projects/00000000-0000-7000-8000-000000000000/keyword-research")" "404" "unknown project is 404"
check "$(code -X DELETE "$BASE/api/v1/projects/$PID/keyword-research/$RID")" "204" "delete research"
api -X DELETE "$BASE/api/v1/projects/$PID" >/dev/null 2>&1 || true
[[ $failed == 0 ]] && echo "e2e-keywords: all checks passed" || { echo "e2e-keywords: FAILED"; exit 1; }

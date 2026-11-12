#!/usr/bin/env python3
"""M5 completion regression; disposable native-AI/SERP fixtures plus real local Ollama.
Usage: python scripts/e2e-m5-final.py API_ORIGIN DB_CONTAINER MODEL_FIXTURE RESEARCH_FIXTURE
Never run against production. SQL intentionally forces schedules and cooldowns.
"""
import http.cookiejar
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid

origin, database, model_fixture, research_fixture = sys.argv[1:5]
def session():
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
owner, outsider, member = session(), session(), session()
def api(path, method="GET", body=None, status=200, client=owner):
    req = urllib.request.Request(origin + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type":"application/json", "x-seoseo-csrf":"1"})
    try:
        response = client.open(req, timeout=75)
    except urllib.error.HTTPError as error:
        response = error
    raw = response.read()
    assert response.status == status, (path, response.status, raw[:1000])
    return json.loads(raw) if raw else None
def sql(statement):
    r = subprocess.run(["docker","exec","-i",database,"psql","-v","ON_ERROR_STOP=1","-U","seoseo","-d","seoseo"], input=statement, text=True, capture_output=True)
    assert r.returncode == 0, r.stderr
    return r.stdout
def control(origin, path="/", body=None):
    req = urllib.request.Request(origin + path, data=json.dumps(body).encode() if body is not None else None, headers={"Content-Type":"application/json"})
    with urllib.request.urlopen(req, timeout=15) as r:
        return json.load(r)
def until(read, accept, timeout=100):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = read()
        if accept(value):
            return value
        time.sleep(.5)
    raise AssertionError(("timed out", value))
for client in (owner, outsider, member):
    api("/auth/sign-up", "POST", {"email":f"m5-final-{uuid.uuid4().hex}@example.com","name":"M5 final","password":"disposable-m5-final-password"},201,client)
me, mate = api("/me"), api("/me",client=member)
sql(f"INSERT INTO members(organization_id,user_id,role) VALUES ('{me['organization_id']}','{mate['user']['id']}','member'); UPDATE sessions SET organization_id='{me['organization_id']}' WHERE user_id='{mate['user']['id']}';")
p = api("/projects","POST",{"name":"M5 final regression","domain":"example.com"},201)
pid = p["id"]
root = "/projects/"+pid
key = api("/api-keys","POST",{"name":"M5 final regression"},201)
def tool(name, args):
    req=urllib.request.Request(origin+"/mcp",method="POST",data=json.dumps({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}).encode(),headers={"Authorization":"Bearer "+key["secret"],"Content-Type":"application/json","Accept":"application/json, text/event-stream"})
    with urllib.request.urlopen(req,timeout=75) as r:
        result=json.load(r)
    value=json.loads(result["result"]["content"][0]["text"])
    assert value["ok"], value
    return value["data"]
models=api("/ai-visibility/models")
for m in ["ollama/smollm2:135m","anthropic/fixture-claude","gemini/fixture-gemini"]:
    assert m in models["models"], models
control(model_fixture,"/control",{})
draft=api(root+"/ai-prompt-research","POST",{"topic":"SEO software","model":"fixture-a"})
assert len(draft["prompts"])==5 and len(control(model_fixture)["calls"])==1
assert api(root+"/ai-trackers")==[]
api(root+"/ai-prompt-research","POST",{"topic":"SEO software"},403,member)
api(root+"/ai-prompt-research","POST",{"topic":"SEO software"},404,outsider)
def tracker(name, selected):
    return api(root+"/ai-trackers","POST",{"expected_domain":"example.com","name":name,"brands":["Acme","Rival"],"prompts":["Which SEO tools are useful?"],"models":selected},201)
t = tracker("Native engines",["ollama/smollm2:135m","anthropic/fixture-claude","gemini/fixture-gemini"])
tpath=root+"/ai-trackers/"+t["id"]
r=api(tpath+"/runs","POST",{"expected_domain":"example.com"},202)
run=until(lambda:api(tpath+"/runs/"+r["id"]),lambda x:x["run"]["status"] in ("completed","failed"),150)
assert run["successful_answers"]==3 and run["run"]["status"]=="completed",run
answers={a["engine"]:a for a in run["answers"]}
assert set(answers)=={"ollama","anthropic","gemini"},answers
assert answers["ollama"]["grounding"]=="none" and answers["ollama"]["citations"]==[]
assert answers["ollama"]["answer"].strip()
assert answers["gemini"]["grounding"]=="none" and answers["gemini"]["citations"]==[]
for engine in ("anthropic",):
    assert answers[engine]["grounding"]=="provider_citations" and len(answers[engine]["citations"])==1,answers[engine]
summary=api(tpath+"/summary")
assert len(summary["trend"])==1 and summary["trend"][0]["successful"]==3 and summary["citation_domains"][0]["links"]==1,summary
assert abs(sum(b["share_of_voice_pct"] or 0 for b in run["brand_rates"])-100)<.01
assert tool("get_ai_visibility_summary",{"project_id":pid,"tracker_id":t["id"]})==summary
scheduled=tracker("Scheduled fixture",["fixture-a"])
spath=root+"/ai-trackers/"+scheduled["id"]
api(spath+"/schedule","PATCH",{"schedule":"weekly","hour_utc":6,"weekday":0},422)
api(spath+"/schedule","PATCH",{"schedule":"weekly","hour_utc":6,"weekday":1},403,member)
for freq in ("daily","weekly","monthly"):
    s=api(spath+"/schedule","PATCH",{"schedule":freq,"hour_utc":6,"weekday":1})
    assert s["next_run_at"] and s["schedule"]==freq
sql(f"UPDATE ai_visibility_trackers SET next_run_at=now()-interval '1 minute' WHERE id='{scheduled['id']}';")
scheduled_view=until(lambda:api(spath),lambda x:len(x["runs"])==1 and x["runs"][0]["status"]=="completed",100)
assert scheduled_view["tracker"]["next_run_at"] > scheduled_view["runs"][0]["created_at"]
# Retention pauses scheduling, preserving stored evidence.
sql(f"INSERT INTO ai_visibility_runs(id,tracker_id,config,gateway_id,status,created_at) SELECT gen_random_uuid(),tracker_id,config,gateway_id,'completed',now()-interval '2 minutes' FROM ai_visibility_runs CROSS JOIN generate_series(1,19) WHERE id='{scheduled_view['runs'][0]['id']}'; UPDATE ai_visibility_runs SET created_at=now()-interval '2 minutes' WHERE tracker_id='{scheduled['id']}'; UPDATE ai_visibility_trackers SET next_run_at=now()-interval '1 minute' WHERE id='{scheduled['id']}';")
paused=until(lambda:api(spath),lambda x:bool(x["tracker"]["schedule_error"]),100)
assert paused["tracker"]["next_run_at"] is None and len(paused["runs"])==20,paused
api(spath+"/schedule","PATCH",{"schedule":"manual","hour_utc":6,"weekday":1})
print("PASS: real Ollama + native Anthropic/Gemini fixture contracts/citations, prompt drafts, rates/trends/MCP, scheduler and retention pause",flush=True)
# Exact SERP + licensed/user-supplied monthly search counts only.
api(root+"/saved-keywords","POST",{"keywords":["m5 volume one","m5 volume two"]})
for kw,pos in [("m5 volume one",1),("m5 volume two",4)]:
    sid=str(uuid.uuid4())
    sql(f"INSERT INTO serp_snapshots(id,provider,exact,keyword,country,language,fetched_at) VALUES ('{sid}','fixture',true,'{kw}','US','en',now()); INSERT INTO serp_items(snapshot_id,position,url,domain,title) VALUES ('{sid}',{pos},'https://competitor.com/page','competitor.com','Result');")
vol={"country":"US","language":"en","source_label":"Synthetic licensed counts, test month","rows":[{"keyword":"m5 volume one","monthly_search_volume":1000},{"keyword":"m5 volume two","monthly_search_volume":800}]}
api(root+"/competitive-volumes","POST",vol,403,member)
api(root+"/competitive-volumes","POST",vol,404,outsider)
api(root+"/competitive-volumes","POST",vol)
overview=api(root+"/domain-overview?domain=competitor.com&limit=1&offset=1")
assert len(overview["keywords"])==1 and overview["matched_keywords"]==2 and overview["estimated_monthly_traffic"]==308,overview
assert overview["traffic_keywords_with_volume"]==2 and overview["keywords"][0]["volume_source"]==vol["source_label"]
sql("UPDATE serp_snapshots SET exact=false WHERE keyword='m5 volume two';")
partial=api(root+"/domain-overview?domain=competitor.com")
assert partial["estimated_monthly_traffic"] is None and partial["partial_sample_estimated_clicks"]==280,partial
assert tool("get_domain_overview",{"project_id":pid,"domain":"competitor.com","limit":1,"offset":1})["keywords"]==partial["keywords"][1:2]
print("PASS: exact-only volume/CTR estimates, partial/unknown states, provenance, keyword pagination, role/tenant and MCP parity",flush=True)
# Bounded search discovery, robots, verified disappearance versus unknown failures.
def discover(mode, search=True):
    control(research_fixture,"/control",{"mode":mode})
    sql(f"UPDATE backlink_discoveries SET created_at=now()-interval '2 minutes' WHERE project_id='{pid}';")
    d=api(root+"/backlink-discoveries","POST",{"expected_domain":"example.com","search":search,"source_urls":[]},202)
    rows=until(lambda:api(root+"/backlink-discoveries"),lambda x:any(r["id"]==d["id"] and r["status"] in ("completed","failed") for r in x))
    d=next(r for r in rows if r["id"]==d["id"])
    assert d["status"]=="completed",d
    snapshot=api(root+"/backlink-snapshots/"+d["snapshot_id"])
    assert ["robots-source.example","/page"] not in control(research_fixture,"/state")["requests"]
    return snapshot
api(root+"/backlink-discoveries","POST",{"expected_domain":"example.com","search":True,"source_urls":[]},403,member)
api(root+"/backlink-discoveries","POST",{"expected_domain":"example.com","search":True,"source_urls":[]},404,outsider)
base=discover("baseline")
assert base["snapshot"]["live_count"]==3 and base["snapshot"]["source"]=="serp_crawl" and base["spam_score"]==40,base
assert all(r["verification"]=="verified" and r["verified_at"] for r in base["items"])
api(root+"/backlink-discoveries","POST",{"expected_domain":"example.com","search":True,"source_urls":[]},409)
blocked=discover("blocked")
assert blocked["snapshot"]["live_count"]==2 and blocked["snapshot"]["lost_count"]==1,blocked
unknown=next(r for r in blocked["items"] if r["target_url"].endswith("/two"))
assert unknown["verification"]=="unconfirmed" and unknown["spam_score"] is None
redirect=discover("redirect",False)
assert next(r for r in redirect["items"] if r["target_url"].endswith("/two"))["verification"]=="unconfirmed"
assert all(path!="/private" for _,path in control(research_fixture,"/state")["requests"])
missing=discover("missing",False)
assert missing["snapshot"]["live_count"]==1 and missing["snapshot"]["lost_count"]==1,missing
lost=next(r for r in missing["items"] if r["target_url"].endswith("/two"))
assert lost["verification"]=="missing" and lost["spam_score"] is None
assert api(root+"/dashboard/backlinks")["latest"]["snapshot"]["id"]==missing["snapshot"]["id"]
assert tool("get_dashboard_backlinks",{"project_id":pid})==api(root+"/dashboard/backlinks")
assert tool("list_backlink_discoveries",{"project_id":pid})==api(root+"/backlink-discoveries")
print("PASS: search crawl/robots, baseline, verified lost links, blocked/redirect unknowns, explainable risk, dashboard and MCP parity",flush=True)

control(research_fixture,"/control",{"mode":"over_limit"})
sql(f"UPDATE backlink_discoveries SET created_at=now()-interval '2 minutes' WHERE project_id='{pid}';")
d=api(root+"/backlink-discoveries","POST",{"expected_domain":"example.com","search":False,"source_urls":["http://source-one.example/page","http://source-two.example/page"]},202)
failed=until(lambda:api(root+"/backlink-discoveries"),lambda x:any(r["id"]==d["id"] and r["status"] in ("completed","failed") for r in x))
assert "1000" in next(r["error"] for r in failed if r["id"]==d["id"])
assert api(root+"/dashboard/backlinks")["latest"]["snapshot"]["id"]==missing["snapshot"]["id"]
print("PASS: over-limit crawl fails without truncating evidence or replacing the previous snapshot",flush=True)

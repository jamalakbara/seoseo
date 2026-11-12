#!/usr/bin/env python3
"""Remaining M4 regression on a disposable password-auth stack and local integration fixture."""
import base64
import concurrent.futures
import datetime
import hashlib
import http.cookiejar
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

origin=sys.argv[1] if len(sys.argv)>1 else "http://127.0.0.1:3300"
fixture="http://127.0.0.1:3101"
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args): return None

def session():return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()),NoRedirect())
owner=session()
def req(path,method="GET",body=None,status=200,client=owner,bearer=None,form=False,csrf=True,headers_only=False,origin_header=None):
    url=path if path.startswith("http") else origin+(path if path.startswith(("/oauth/","/.well-known/","/mcp")) else "/api/v1"+path)
    headers={"Content-Type":"application/x-www-form-urlencoded" if form else "application/json","Accept":"application/json, text/event-stream"}
    if csrf:headers["x-seoseo-csrf"]="1"
    if origin_header:headers["Origin"]=origin_header
    if bearer:headers["Authorization"]="Bearer "+bearer
    data=None if body is None else (urllib.parse.urlencode(body) if form else json.dumps(body)).encode()
    try:r=client.open(urllib.request.Request(url,method=method,headers=headers,data=data),timeout=50)
    except urllib.error.HTTPError as e:r=e
    raw=r.read()
    assert r.status==status,(path,r.status,raw[:500])
    if headers_only:return r.headers
    if status in (302,303):return r.headers["Location"]
    return json.loads(raw) if raw and r.headers.get_content_type()=="application/json" else raw

def signup(c):return req("/auth/sign-up","POST",{"email":"final-"+uuid.uuid4().hex+"@example.com","password":"disposable-final-regression-password","name":"M4 final"},201,client=c)
def control(mode):return req(fixture+"/control","POST",{"mode":mode})
def mcp(name,args,bearer):
    data=req("/mcp","POST",{"jsonrpc":"2.0","id":uuid.uuid4().hex,"method":"tools/call","params":{"name":name,"arguments":args}},bearer=bearer)
    return json.loads(data["result"]["content"][0]["text"])

signup(owner)
other=session();signup(other)
p=req("/projects","POST",{"name":"M4 final integration regression","domain":"badseo.test"},201);pid=p["id"];root="/projects/"+pid
key=req("/api-keys","POST",{"name":"M4 final"},201)
try:
    # Separate Google grants and paginated selection, all nine fixed report paths.
    control("normal")
    req(root+"/ga4",client=other,status=404)
    assert not req(root+"/ga4")["connected"]
    req(root+"/ga4/connect","POST",bearer=key["secret"],status=403)
    for provider in ("gsc","ga4"):
        start=req(root+"/"+provider+"/connect","POST")["url"]
        q=urllib.parse.parse_qs(urllib.parse.urlsplit(start).query)
        assert q["scope"]==["https://www.googleapis.com/auth/"+("analytics.readonly" if provider=="ga4" else "webmasters.readonly")]
        callback=req(start,status=302)
        parsed=urllib.parse.urlsplit(callback)
        url=origin+parsed.path+"?"+parsed.query
        req(url,status=422,client=other)
        req(url,status=303)
        req(url,status=422)
    props=req(root+"/ga4/properties");assert props["next_page_token"]=="page2"
    assert req(root+"/ga4/properties?page_token=page2")["properties"][0]["property"]=="properties/222"
    req(root+"/ga4/property","PUT",{"property_id":"properties/999"},422)
    req(root+"/ga4/property","PUT",{"property_id":"properties/222"})
    today=datetime.datetime.now(datetime.timezone.utc).date()
    query={"start_date":str(today-datetime.timedelta(days=30)),"end_date":str(today-datetime.timedelta(days=2)),"limit":100}
    names=["organic_overview","organic_landing_pages","page_performance","traffic_acquisition","key_events","ecommerce_performance","site_search","audience_breakdown","measurement_health"]
    for name in names:
        report=req(root+"/ga4/reports/"+name,"POST",query)
        assert report["property_id"]=="properties/222" and report["data"]["rows"]
        assert report["channel"]==("all" if name in ["traffic_acquisition","site_search","audience_breakdown","measurement_health"] else "organic_search")
        assert mcp("get_google_analytics_"+name,{"project_id":pid,**query},key["secret"])["ok"]
    req(root+"/ga4/reports/organic_overview","POST",{**query,"limit":1001},422)
    req(root+"/ga4/reports/organic_overview","POST",{**query,"metrics":["arbitrary"]},422)
    control("restricted");restricted=req(root+"/ga4/reports/ecommerce_performance","POST",query)
    assert restricted["data"]["rows"][0]["metricValues"][0]["value"] is None
    control("empty");assert not req(root+"/ga4/reports/organic_overview","POST",query)["data"].get("rows")
    control("quota");req(root+"/ga4/reports/organic_overview","POST",query,503)
    control("normal");req(root+"/ga4","DELETE",status=204)
    assert req(root+"/gsc")["connected"] and not req(root+"/ga4")["connected"]
    print("PASS GA4: independent OAuth, state binding/replay, pagination, verified property selection, nine REST/MCP reports, bounds, privacy, empty/quota, disconnect isolation",flush=True)

    # Optional model output is a draft; it must never write memory by itself.
    req(root+"/website-synthesis","POST",status=409)
    audit=req(root+"/website-research","POST",{"start_url":"http://badseo.test/"})
    for _ in range(60):
        setup=req(root+"/website-setup")
        if setup["audit"]["status"]=="completed":break
        time.sleep(1)
    assert setup["evidence"]
    draft=req(root+"/website-synthesis","POST")
    assert draft["business_overview"] and req(root+"/context")["revision"]==0
    control("model-invalid");req(root+"/website-synthesis","POST",status=503)
    assert req(root+"/context")["revision"]==0
    control("normal")
    req(root,"PATCH",{"domain":"example.test"})
    link={"source_url":"https://source.example/article","target_url":"https://example.test/service","state":"live"}
    req(root+"/links","POST",link,client=other,status=404)
    req(root+"/links","POST",{**link,"target_url":"https://evil.test/"},422)
    observed=req(root+"/links","POST",link)
    duplicate=req(root+"/links","POST",link)
    assert observed["id"]==duplicate["id"] and observed["observed_at"]==duplicate["observed_at"]
    req(root+"/links","POST",{**link,"state":"lost"})
    assert req(root+"/dashboard/overview")["link_activity"][0]["state"]=="lost"
    req(root,"PATCH",{"domain":"other.test"})
    assert req(root+"/dashboard/overview")["link_activity"]==[]
    req(root,"PATCH",{"domain":"example.test"})
    print("PASS model/link activity: bounded evidence, invalid-output failure, no implicit memory writes, tenant/target validation, deduplication and dashboard state",flush=True)

    # Key setup is atomic. Ownership is exact-host, no redirects, no private fetch bypass.
    with concurrent.futures.ThreadPoolExecutor(max_workers=5) as pool:
        settings=list(pool.map(lambda _:req(root+"/indexnow/setup","POST"),range(5)))
    assert len({s["key"] for s in settings})==1
    assert req(root+"/indexnow/verify","POST")["verified"]
    req(root+"/indexnow/submit","POST",{"urls":["https://example.test.evil.test/a"]},422)
    req(root+"/indexnow/submit","POST",{"urls":["http://127.0.0.1/private"]},422)
    control("key-redirect");assert not req(root+"/indexnow/verify","POST")["verified"]
    control("key-missing");assert not req(root+"/indexnow/verify","POST")["verified"]
    control("normal")
    results=req(root+"/indexnow/submit","POST",{"urls":["http://example.test/a","http://example.test/a","http://www.example.test/b"]})
    assert len(results)==2 and all(r["accepted"] and r["status"]==202 for r in results)
    req(root+"/indexnow/submit","POST",{"urls":["http://example.test/a"]},409)
    print("PASS IndexNow: atomic setup, host boundaries, no-redirect/missing-key verification, host grouping, deduplication, accepted-vs-indexed semantics and cooldown",flush=True)

    # OAuth discovery, public registration, explicit session consent and one-resource tokens.
    anonymous=session()
    challenge=req("/mcp","POST",{"jsonrpc":"2.0","id":1,"method":"tools/list"},status=401,client=anonymous,headers_only=True)
    assert "resource_metadata" in challenge["WWW-Authenticate"]
    resource=origin+"/mcp"
    assert req("/.well-known/oauth-protected-resource/mcp")["resource"]==resource
    assert req("/.well-known/oauth-authorization-server")["code_challenge_methods_supported"]==["S256"]
    for path,allowed in [("/oauth/token",True),("/mcp",True),("/api/v1/oauth/consent",False)]:
        r=urllib.request.Request(origin+path,method="OPTIONS",headers={"Origin":"https://client.example","Access-Control-Request-Method":"POST","Access-Control-Request-Headers":"content-type"})
        with urllib.request.urlopen(r) as response:
            assert (response.headers.get("Access-Control-Allow-Origin")=="https://client.example")==allowed

    req("/oauth/register","POST",{"redirect_uris":["http://evil.example/callback"]},400,client=anonymous)
    registration=req("/oauth/register","POST",{"client_name":"M4 regression client","redirect_uris":["http://127.0.0.1:49199/callback"],"token_endpoint_auth_method":"none"},201,client=anonymous)
    client_id=registration["client_id"]
    verifier=base64.urlsafe_b64encode(hashlib.sha256(uuid.uuid4().bytes).digest()).decode().rstrip("=")
    challenge=base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
    authorization={"client_id":client_id,"redirect_uri":registration["redirect_uris"][0],"response_type":"code","code_challenge":challenge,"code_challenge_method":"S256","resource":resource,"scope":"mcp","state":"opaque-client-state"}
    req("/oauth/authorize?"+urllib.parse.urlencode({**authorization,"redirect_uri":"https://evil.example/"}),status=400,client=anonymous)
    req("/oauth/authorize?"+urllib.parse.urlencode(authorization),status=303,client=anonymous)
    # /oauth/* public routing and /api/v1/oauth/* authenticated REST are distinct.
    consent_path=origin+"/api/v1/oauth/consent"
    req(consent_path,"POST",{"authorization":authorization,"allow":True},403,csrf=False)
    req(consent_path,"POST",{"authorization":authorization,"allow":True},403,bearer=key["secret"])
    denied=req(consent_path,"POST",{"authorization":authorization,"allow":False})
    assert urllib.parse.parse_qs(urllib.parse.urlsplit(denied["redirect_uri"]).query)["error"]==["access_denied"]
    def authorize():
        consent=req(consent_path,"POST",{"authorization":authorization,"allow":True})
        return urllib.parse.parse_qs(urllib.parse.urlsplit(consent["redirect_uri"]).query)["code"][0]
    def exchange(code,proof=verifier,status=200):
        return req("/oauth/token","POST",{"grant_type":"authorization_code","client_id":client_id,"code":code,"redirect_uri":authorization["redirect_uri"],"code_verifier":proof,"resource":resource},status,client=anonymous,form=True)
    code=authorize();exchange(code,"wrong"*10,400);exchange(code,status=400)
    code=authorize();pair=exchange(code)
    assert pair["expires_in"]==3600 and mcp("whoami",{},pair["access_token"])["ok"]
    req("/mcp","POST",{},401,origin_header="https://client.example")
    cross=req("/mcp","POST",{"jsonrpc":"2.0","id":1,"method":"tools/list"},bearer=pair["access_token"],origin_header="https://client.example",headers_only=True)
    assert cross["Access-Control-Allow-Origin"]=="https://client.example"

    req("/projects",bearer=pair["access_token"],status=401)
    isolated=mcp("get_dashboard_overview",{"project_id":pid},pair["access_token"]);assert isolated["ok"]
    exchange(code,status=400)
    req("/mcp","POST",{},status=401,bearer=pair["access_token"])
    pair=exchange(authorize())
    refresh={"grant_type":"refresh_token","client_id":client_id,"refresh_token":pair["refresh_token"],"resource":resource}
    req("/oauth/token","POST",{**refresh,"resource":"https://evil.example/mcp"},400,client=anonymous,form=True)
    rotated=req("/oauth/token","POST",refresh,client=anonymous,form=True)
    assert rotated["refresh_token"]!=pair["refresh_token"] and mcp("whoami",{},rotated["access_token"])["ok"]
    req("/oauth/token","POST",refresh,400,client=anonymous,form=True)
    req("/mcp","POST",{},401,bearer=rotated["access_token"])
    pair=exchange(authorize())
    grants=req(origin+"/api/v1/oauth/grants");assert grants
    req(origin+"/api/v1/oauth/grants/"+grants[0]["id"],"DELETE",status=204,client=other)
    assert mcp("whoami",{},pair["access_token"])["ok"]
    req("/oauth/revoke","POST",{"client_id":client_id,"token":pair["refresh_token"]},client=anonymous,form=True)
    req("/mcp","POST",{},401,bearer=pair["access_token"])
    # Membership is resolved on every token use; a removed member loses access immediately.
    email="member-"+uuid.uuid4().hex+"@example.com"
    invitation=req("/team/invitations","POST",{"email":email,"role":"member"},201)
    member=session();req("/auth/sign-up","POST",{"email":email,"password":"disposable-member-password","name":"OAuth member","invite_token":invitation["token"]},201,client=member)
    member_id=req("/me",client=member)["user"]["id"]
    member_consent=req(consent_path,"POST",{"authorization":authorization,"allow":True},client=member)
    member_code=urllib.parse.parse_qs(urllib.parse.urlsplit(member_consent["redirect_uri"]).query)["code"][0]
    member_token=exchange(member_code)
    assert mcp("get_dashboard_overview",{"project_id":pid},member_token["access_token"])["ok"]
    assert not mcp("save_link_observation",{"project_id":pid,**link},member_token["access_token"])["ok"]
    req(root+"/ga4/connect","POST",client=member,status=403)
    req(root+"/indexnow/setup","POST",client=member,status=403)
    req("/team/members/"+member_id,"DELETE",status=204)
    req("/mcp","POST",{},401,bearer=member_token["access_token"])
    print("PASS MCP OAuth: discovery/DCR, exact redirect/resource, PKCE, CSRF/session consent, denial, code replay revocation, REST exclusion, refresh rotation/replay and scoped revocation",flush=True)
finally:
    control("normal")
    req("/api-keys/"+key["key"]["id"],"DELETE",status=204)
    req(root+"/archive","POST")
    req(root,"DELETE",status=204)

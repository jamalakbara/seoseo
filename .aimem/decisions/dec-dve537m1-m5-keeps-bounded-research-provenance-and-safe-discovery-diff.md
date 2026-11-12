---
aimem: 1
id: dec-dve537m1
type: decision
title: M5 keeps bounded research provenance and safe discovery diffs
summary: Crawl bounded SERP/explicit source pages; preserve blocked links as unconfirmed. Estimate sample clicks only from exact ranks and imported search counts.
status: proposed
trust: verified
roles: [all]
tags: [m5 competitive backlinks]
author: human:akbar
created: 2026-11-12T23:10:29Z
updated: 2026-11-12T23:10:29Z
verified: 2026-11-12
supersedes: dec-6qn195jg
---

M5 extends project scope to saved/researched/rank-tracked/volume-imported keywords (500 cap). Exact fresh SERPs plus labelled imported monthly search counts produce observed-sample CTR estimates; never substitute Wikipedia pageviews or claim total domain traffic. Default backlink discovery is a bounded SERP/explicit-source guarded HTML crawl (40 pages, 1000 current pairs), not a Common Crawl target-prefix query: a source URL index does not provide target inlinks. Imported and crawl histories are separate; robots refusals, blocks, redirects, timeouts and extraction caps preserve previous evidence as unconfirmed. Only definitive source reads establish missing pairs. Scores are explainable HTML heuristics, unknown for imports or inconclusive rows.

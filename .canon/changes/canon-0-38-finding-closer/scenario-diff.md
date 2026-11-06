# Scenarios whose sentence changed (`canon-0-38-finding-closer`)

The scenarios of the specs whose title, Given, When or Then differ between `main` at 172e43ab8 and this change. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `finding.closer.01-12` (`area=finding/closer.feature`) are not in the table.

Scenarios changed: 2

| scenario | file | what changed |
|---|---|---|
| @finding.list.01 | area=finding/list.feature | the table has a `closed by` column between `reviewer` and `summary` (“… reviewer and summary” → “… reviewer, the closer and summary”): the actor of the closing version and its role, `-` while the finding is open |
| @atip.claim14.15 | area=atip/claim14.feature | a live answer is not larger than the stated 0.36.0 size and within a tenth of it, except `schema finding close` and `schema finding list`, which gained members after 0.36.0 and may exceed it by at most 10% each (the allowance is listed in the scenario and in the test) |

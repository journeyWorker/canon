# Scenarios whose sentence changed (`canon-0-38-finding-closer`)

The scenarios of the specs whose title, Given, When or Then differ between `main` at 172e43ab8 and this change. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `finding.closer.01-12` (`area=finding/closer.feature`) are not in the table.

Scenarios changed: 1

| scenario | file | what changed |
|---|---|---|
| @finding.list.01 | area=finding/list.feature | the table has a `closed by` column between `reviewer` and `summary` (“… reviewer and summary” → “… reviewer, the closer and summary”): the actor of the closing version and its role, `-` while the finding is open |

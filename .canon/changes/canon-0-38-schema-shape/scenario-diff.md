# Scenarios whose sentence changed (`canon-0-38-schema-shape`)

The scenarios of the specs whose title, Given, When or Then differ between `main` at 75db5178e and this change. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `atip.schemashape.01-12` (`area=atip/schemashape.feature`) are not in the table.

Scenarios changed: 1

| scenario | file | what changed |
|---|---|---|
| @atip.schemadoc.01 | area=atip/schema.feature | the Then said "the unfiltered trees have no x-canon"; it says every tree, filtered or not, has `x-canon.rolesForm` (the two forms of `x-canon.roles`, said once at the root) and an unfiltered tree has no other `x-canon` member, and that each command entry carries `x-canon.roles` as `all` or an array of role names |

Two things no scenario sentence says changed: the value name of the `trace` operand is `TRACE_ID`, so the `field` of the refusals of `canon trace` (`TRACE_OPERAND_INVALID`, `TRACE_NOT_FOUND`, `TRACE_CONFLICT`) is `TRACE_ID` where it was `ID` (the sentences of `search.trace.04` and `atip.contract11` name the codes, not the field), and the message of `TRACE_OPERAND_INVALID` names `TRACE_ID` where it named `ID`; `skills.install.21` (only the three human acts name no agent role) reads the empty array, which is as it was.

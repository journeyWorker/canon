# Scenarios whose sentence changed (`canon-0-37-option-forms`)

The scenarios of the specs whose title, Given, When or Then differ between `main` at a9e724f90 and this change. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `atip.optionforms.01-16` (`area=atip/optionforms.feature`) are not in the table.

Scenarios changed: 7

| scenario | file | what changed |
|---|---|---|
| @atip.optionvalues.12 | area=atip/optionvalues.feature | one name is allowed a different domain or set in some commands (`--kind`), no longer two: `--providers` has one domain in `init` and the three `skills` commands |
| @atip.optionvalues.13 | area=atip/optionvalues.feature | the sweep finds `--kind` when it is not allowed and passes the real documents with the one allowed (it named `--providers` too) |
| @atip.claim15.03 | area=atip/claim15.feature | the 59 places: 29 keep the property spelling, 17 hold one of the six name and sentence options, and the 13 places of the time options and the provider list say the RFC 3339 and exact-name patterns, which name no white space; the distinct patterns that name white space are 11 (13 before) |
| @atip.claim15.04 (two scenarios) | area=atip/claim15.feature | the comparison with the patterns of 0.36.0 covers the 29 places that kept their values (the 13 places of the time options and the provider list are held to `atip.optionforms`) |
| @atip.claim15.08 | area=atip/claim15.feature | 11 distinct property patterns (13 before) |
| @atip.constraints.07 | area=atip/constraints.feature | `--approval-at` takes a date-time, so the impossible date it is given is `2026-02-30T10:00:00Z`; the five date options are still given `2026-02-30`; the 12 time options are named by the calendar validator of their command as before |
| @authoring.init.93 | area=authoring/init.feature | an empty `--providers` was an explicit empty choice; it is refused with exit 2 now (`--providers none` is the explicit empty choice) and nothing is written |

`crates/canon-cli/tests/atip/white_space_0_36.json` is re-baselined in one way: the 13 places of the seven options of this change are listed under `changedBy037Forms` and are held to the patterns of `atip.optionforms.11`. The patterns and places of 0.36.0 stay in the file as the record.

Two things no scenario sentence says changed: `atip.constraints.05`'s tables of values just inside and outside each domain now hold the RFC 3339 spellings (the shared `TIMESTAMP` row, the `--approval-at` row and the two `--providers` rows); and the validator of `--approval-at` in four commands says the date-time (`CALENDAR_DATE_TIME`), with a rejected call that is a date-time which does not exist.

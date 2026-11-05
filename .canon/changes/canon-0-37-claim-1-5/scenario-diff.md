# Scenarios whose sentence changed (`canon-0-37-claim-1-5`, F6)

The scenarios of the specs whose title, Given, When or Then differ between `main` at ab16057f0 and this change: they said the 1.4 declaration. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `atip.claim15.01-15` (`area=atip/claim15.feature`) are not in the table.

Scenarios changed: 17

| scenario | file | what changed |
|---|---|---|
| @atip.check.43 | area=atip/check.feature | “1.4,” → “1.5,” |
| @atip.claim14.01 | area=atip/claim14.feature | “1.4,” → “1.5,”; “1.4” → “1.5”; “1.4,” → “1.5,” |
| @atip.claim14.11 | area=atip/claim14.feature | “1.4” → “1.5” |
| @atip.claim14.13 | area=atip/claim14.feature | “1.4,” → “1.5,”; “1.3” → “1.4”; “1.4” → “1.5” |
| @atip.claim14.14 | area=atip/claim14.feature | “1.4” → “1.5”; “1.4” → “1.5”; “1.4” → “1.5” |
| @atip.claim14.15 | area=atip/claim14.feature | “Unreleased” → “0.36.0”; “this” → “the 0.36.0”; “equals” → “is”; “measure,” → “0.36.0 measure (a live answer is not larger than it and lies within a tenth of it, the patterns of 0.37 being shorter),” |
| @atip.claiml2.01 | area=atip/claiml2.feature | “1.4,” → “1.5,”; “1.4” → “1.5”; “1.4” → “1.5”; “1.4,” → “1.5,” |
| @atip.claiml2.07 | area=atip/claiml2.feature | “1.4” → “1.5”; “1.4,” → “1.5,” |
| @atip.claiml2.09 | area=atip/claiml2.feature | “1.4” → “1.5”; “1.4,” → “1.5,” |
| @atip.claiml2.19 | area=atip/claiml2.feature | “1.4,” → “1.5,” |
| @atip.claiml2.20 | area=atip/claiml2.feature | “1.4” → “1.5” |
| @atip.contract11.01 | area=atip/contract11.feature | “” → “in 0.36, 1.5”; “0.36),” → “0.37),”; “1.4” → “1.5”; “1.4” → “1.5” |
| @atip.contract11.05 | area=atip/contract11.feature | “1.4,” → “1.5,” |
| @atip.crate12.12 | area=atip/crate12.feature | “1.4,” → “1.5,” |
| @atip.crate12.14 | area=atip/crate12.feature | “1.4” → “1.5” |
| @atip.schemadoc.01 | area=atip/schema.feature | “1.4;” → “1.5;” |
| @atip.schemadoc.13 | area=atip/schema.feature | “” → “says contractVersion 1.5 and”; “1.4” → “1.4-form” |

Also changed, not a scenario: the `Feature:` lines of `claim14.feature` and `claiml2.feature` say that 0.36 declared 1.4 and 0.37 declares 1.5.

`atip.claim14.15` is reworded for a reason beyond the declaration: its test measured the sizes of the answers of the build under test and held the 0.36.0 entry of the CHANGELOG to them. The patterns of this build are shorter (the 59 declarations are 4,894 characters now, 1,930 fewer than the 6,824 of 0.36.0), so the answers of this build are smaller than the sizes the 0.36.0 entry states. The test holds the entry's sizes to the 0.36.0 build as follows: a live answer is not larger than the stated size and lies within a tenth of it. The table, the sum rows and the callers paragraph are checked as before.

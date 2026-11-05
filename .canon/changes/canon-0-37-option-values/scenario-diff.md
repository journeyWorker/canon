# Scenarios whose sentence changed (`canon-0-37-option-values`)

The scenarios of the specs whose title, Given, When or Then differ between `main` at 64924a3a7 and this change. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `atip.optionvalues.01-16` (`area=atip/optionvalues.feature`) are not in the table.

Scenarios changed: 8

| scenario | file | what changed |
|---|---|---|
| @finding.close.05 | area=finding/close.feature | “A fix that did not hold reopens the finding” → “… is a new finding, and closing a finding to open is refused”: closing to open is a usage error naming --disposition and the three dispositions, nothing is recorded, and the defect that came back is a new finding (the brief decides: no reopening) |
| @finding.close.88 | area=finding/close.feature | “Closing or reopening …” → “Closing a finding twice …”; “closed fixed and then reopened” → “closed fixed and then closed rejected” |
| @finding.close.89 | area=finding/close.feature | “still closes and reopens” → “still closes twice”; “closed fixed and then reopened” → “closed fixed and then closed rejected” |
| @atip.constraints.14 | area=atip/constraints.feature | `--approver` of brief approve and policy approve now declares the one domain of a name with APPROVAL_INVALID; it had no domain and a validator. `--note` keeps no domain and its validator |
| @atip.checkorder.18 | area=atip/checkorder.feature | the blank approver is the domain's (alone and beside the constraint), the validator's fault is the blank `--note`; the constraint answers USAGE beside the validator's fault as before |
| @atip.claim15.03 | area=atip/claim15.feature | the 59 places: 42 keep the property spelling and are held to 0.36.0, 17 hold one of the six name and sentence options and say the one text pattern; the distinct patterns that name white space are 13 (17 before) |
| @atip.claim15.04 (two scenarios) | area=atip/claim15.feature | the comparison with the patterns of 0.36.0 covers the 42 places that kept their values (the 17 of the six options are held to `atip.optionvalues`); the one-word token is compared as its own spelling, since it moved from the domain of `--relayed-via` to the validator beside it |
| @atip.claim15.08 | area=atip/claim15.feature | 13 distinct property patterns (17 before); a pattern that has a 0.36.0 pattern takes and refuses the values it did |

`crates/canon-cli/tests/atip/white_space_0_36.json` is re-baselined in one way: the 17 places of the six name and sentence options are listed under `changedBy037` and are held to the text rule of this change. The patterns and places of 0.36.0 stay in the file as the record.

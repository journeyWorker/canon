# Scenarios whose sentence changed (`canon-0-36-claim-1-4`, F5)

The scenarios of the specs whose title, Given, When or Then differ between `main` at 083f95f01 and this change: they said the 1.2 declaration or the form of the 1.3 self-description. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `atip.claim14.01-16` (`area=atip/claim14.feature`) are not in the table.

Scenarios changed: 11

| scenario | file | what changed |
|---|---|---|
| @atip.check.43 | area=atip/check.feature | “1.2,” → “1.4,” |
| @atip.claiml2.01 | area=atip/claiml2.feature | “1.2,” → “1.4,”; “1.2” → “1.4”; “1.2” → “1.4”; “1.2,” → “1.4,” |
| @atip.claiml2.02 | area=atip/claiml2.feature | “1.1” → “1.1, 1.2 or 1.3”; “” → “1.2 or 1.3,”; “1.1” → “an earlier minor”; “a 1.1” → “an earlier” |
| @atip.claiml2.07 | area=atip/claiml2.feature | “1.2” → “1.4”; “1.2,” → “1.4,” |
| @atip.claiml2.09 | area=atip/claiml2.feature | “1.2” → “1.4”; “1.2,” → “1.4,” |
| @atip.contract11.01 | area=atip/contract11.feature | “(1.2” → “(1.4”; “0.35),” → “0.36),”; “1.2” → “1.4”; “1.2” → “1.4” |
| @atip.contract11.05 | area=atip/contract11.feature | “1.2,” → “1.4,” |
| @atip.schemadoc.01 | area=atip/schema.feature | “L1” → “L2”; “L1;” → “L2;”; “commands;” → “commands and no entry has path or kind command; meta.schemaVersion and contractVersion are 1.4;” |
| @atip.schemadoc.07 | area=atip/schema.feature | “input commands list” → “declaration lists”; “INPUT_TOO_LARGE,” → “INPUT_TOO_LARGE for the input commands, CURSOR_INVALID for”; “CURSOR_INVALID,” → “and PLAN_STALE for”; “ones PLAN_STALE;” → “ones, and the 1.4 document omits those implied codes;”; “” → “in the --schemas answer” |
| @atip.schemadoc.09 | area=atip/schema.feature | “schemas,” → “Input of the --schemas answer and the arguments schema,” |
| @atip.schemadoc.13 | area=atip/schema.feature | “” → “1.4”; “byte;” → “byte, and under schemas the --schemas answer of each command; the tree defines each shared option once and every 1.4 rule of the form holds;” |

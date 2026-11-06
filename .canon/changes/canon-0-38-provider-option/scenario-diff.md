# Scenarios whose sentence changed (`canon-0-38-provider-option`)

The scenarios of the specs whose title, Given, When or Then differ between `main` at 251e49300 and this change. Their ids, tags and flows are unchanged. A review of one of them was pinned to the old sentence and is stale; a reviewer other than the author reads the table and signs again. The new scenarios `atip.provideroption.01-15` (`area=atip/provideroption.feature`) are not in the table.

Scenarios changed: 22

| scenario | file | what changed |
|---|---|---|
| @authoring.init.90 | area=authoring/init.feature | “no --providers” → “neither --harness nor --no-harness” |
| @authoring.init.91 | area=authoring/init.feature | the title and the steps say `--harness codex` (it was `--providers codex`); a rerun with `--harness` says to run `canon skills install` with it |
| @authoring.init.92 | area=authoring/init.feature | a bare repository is told `canon skills install --harness claude` and `--no-harness` (it was `canon skills install --providers=<list>`) |
| @authoring.init.93 | area=authoring/init.feature | the explicit empty choice is `--no-harness` (it was `--providers none`); an empty `--harness` is refused with exit 2 and writes nothing (it was an empty `--providers`) |
| @authoring.init.94 | area=authoring/init.feature | an unknown harness in `--harness` is refused with `USAGE` naming `--harness` and the four names as `expected` (it was `INIT_ARGUMENT_INVALID` naming `--providers` and the accepted names of its domain) |
| @authoring.init.95 | area=authoring/init.feature | “no --providers” → “neither --harness nor --no-harness” |
| @authoring.init.96 | area=authoring/init.feature | “no --providers” → “neither --harness nor --no-harness”; with `--harness` or `--no-harness` the choice is the answer |
| @skills.install.51 | area=skills/install.feature | a target with no harness is told `canon skills install --harness claude` and `--no-harness` (it was `--providers=<list>`) |
| @skills.install.52 | area=skills/install.feature | an explicit list is written by `--harness codex` and `--harness omp` (it was `--providers codex,omp`); a later `--harness` replaces the section and `--no-harness` records an empty list (it was `--providers none`) |
| @skills.install.55 | area=skills/install.feature | an unknown harness on the `--harness` option of a clean canon.yaml is `USAGE`, refused by the enumeration before any section is read (it was `SKILLS_ARGUMENT_INVALID` on `--providers`) |
| @skills.install.54 | area=skills/install.feature | the title and the stray line of the doctor say a harness that is not selected (review c76310f34 note 1; it said a provider) |
| @skills.install.67 | area=skills/install.feature | `canon init --harness claude --harness codex` (it was `--providers claude,codex`) |
| @atip.codesplit.04 | area=atip/codesplit.feature | the bad harness is `skills install --harness bogus`, which is `USAGE` (it was `--providers bogus` with `SKILLS_ARGUMENT_INVALID`) |
| @atip.midway.06 | area=atip/midway.feature | the refusal of an unknown harness is `canon init --harness` with `USAGE` (it was `--providers` with `INIT_ARGUMENT_INVALID`); it is still no `INIT_INCOMPLETE` and writes nothing |
| @atip.optionvalues.12 | area=atip/optionvalues.feature | the note that `--providers` has one domain is gone: `--harness` declares no domain, it is an enumeration |
| @atip.optionforms.09 | area=atip/optionforms.feature | the four commands take `--no-harness` and each exact name given with `--harness`, as the lists of 0.37 were taken (it took `none` and the comma lists of `--providers`) |
| @atip.optionforms.10 | area=atip/optionforms.feature | a name that is not exact (a capital, `none`, white space, two names in one value, an empty value, an unknown name) is refused by the four commands with `USAGE` naming `--harness` and the four names (it was a list that was not exact, with `INIT_ARGUMENT_INVALID` and `SKILLS_ARGUMENT_INVALID` naming `--providers`) |
| @atip.optionforms.11 | area=atip/optionforms.feature | six options, not seven: the four commands that took `--providers` are out, and the help names the date-time and the RFC 3339 date (it named the lowercase names too) |
| @atip.optionforms.15 | area=atip/optionforms.feature | twelve places of six options (it was sixteen places of seven) |
| @atip.optionforms.16 | area=atip/optionforms.feature | each place of the six options was measured, and the entries of `--providers` are kept under `supersededBy038` as the record |
| @atip.claim15.03 | area=atip/claim15.feature | the 13 places of the time options and the harness list are 11 places of the time options, and the 2 places of `--providers` are no declaration any more (the option was removed) |
| @atip.claim15.04 | area=atip/claim15.feature | the places held to `atip.optionforms` are the 11 of the time options; the 2 places of `--providers` are not declared in 0.38 |

`crates/canon-cli/tests/atip/option_forms_0_36.json` is re-baselined in one way: the entries of `--providers` (19 accepted, 5 refused, and the nine classes only they use) are under `supersededBy038` and the six other options are as they were; `white_space_0_36.json` lists the two places of `--providers` under `changedBy038` and the 13 places of `changedBy037Forms` are 11. The 0.37 CHANGELOG keeps its rows for `--providers` as that release's notes; the 0.38 Breaking table has the rows of the new change.

Two things no scenario sentence says changed: the reason `SKILLS_ARGUMENT_INVALID` is gone from `INIT_INCOMPLETE` (the tables of `atip.incomplete` and `atip.reasons` no longer list it), and the sites survey of `atip.constraints` classifies the one `SKILLS_ARGUMENT_INVALID` site that is left (an install lock that names an unknown provider) as a precondition, not as the domain of `--providers`.

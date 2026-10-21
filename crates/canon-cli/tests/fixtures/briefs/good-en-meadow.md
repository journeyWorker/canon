# Recipe scaling: unit conversion

<!-- canon:section intent role=context -->
## Intent

Cooks who double a recipe get quantities such as 1.5 tablespoons that no spoon measures. Round scaled amounts to a unit a cook can measure.

<!-- canon:section outcome role=normative -->
## Desired outcome

A cook who scales a recipe sees each amount in the nearest practical unit. ==**Scaling never changes the ratio between ingredients by more than two percent.**==

<!-- canon:section flow role=normative -->
## User flow

1. F1 The cook sees a recipe with a scale control → sets the scale to 2 → sees every amount doubled and rounded to a measurable unit. ==**The rounded amounts appear immediately.**==
2. F2 The cook sees an amount that cannot be rounded within two percent → keeps the scale → sees the exact amount with an "about" label.

<!-- canon:section detail role=normative -->
## Step detail

### F1 Rounding

- Amounts under three teaspoons are shown in teaspoons, up to a quarter cup in tablespoons, and above that in cups.
- Eggs round to the nearest half. ==**A rounded amount is never shown with more than two fractional digits.**==

### F2 Exact amounts

- An amount that rounding would shift by more than two percent keeps its exact value.
- The label reads "about" and nothing else changes in the row.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: scaling by 2 shows 2 tablespoons where the recipe had 1, and 3 tablespoons where it had 1.5.
- F2: an amount that rounds badly shows the exact value and the "about" label. ==**No amount is ever dropped or merged.**==

<!-- canon:section questions role=question -->
## Questions and assumptions

None.

<!-- canon:section out-of-scope role=context -->
## Out of scope

Metric and imperial switching.

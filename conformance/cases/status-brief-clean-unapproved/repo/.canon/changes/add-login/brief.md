# Add login

<!-- canon:section intent role=context -->
## Intent

Add login

Visitors lose their cart when they switch devices.

<!-- canon:section outcome role=normative -->
## Desired outcome

A visitor who signs in sees the saved cart. ==**The cart follows the account.**==

<!-- canon:section flow role=normative -->
## User flow

1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==
2. F2 The visitor submits a wrong password → sees an error → can retry.

<!-- canon:section detail role=normative -->
## Step detail

### F1 Submit

The server issues the session. ==**The password is never logged.**==

### F2 Failed submit

Five failures lock the form for thirty seconds.

<!-- canon:section acceptance role=normative -->
## Acceptance

- F1: valid credentials open the catalog and set a session cookie.
- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==

<!-- canon:section questions role=question -->
## Questions and assumptions

None.

<!-- canon:section out-of-scope role=context -->
## Out of scope

Social sign-in.

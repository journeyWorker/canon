# Shared calendar: busy-time preview

## Intent

Team members pick meeting times without seeing who is busy. Show each invitee's busy blocks while the organizer chooses a slot.

## Desired outcome

An organizer who opens the slot picker sees every invitee's busy blocks and can pick a free slot in one pass. ==**The preview shows busy or free, never the event title.**==

## User flow

1. F1 The organizer sees the slot picker with invitees → adds an invitee → sees that person's busy blocks appear. ==**Busy blocks appear within two seconds of adding the invitee.**==
2. F2 The organizer adds an invitee who has not shared a calendar → sees a "No calendar shared" label → can still pick a slot.

## Step detail

### F1 Busy blocks

- Blocks are drawn in the organizer's time zone.
- Two invitees busy at the same time show one merged block. ==**A block never carries an event title.**==

### F2 No shared calendar

- The invitee row shows the label and no blocks.
- The organizer's pick is saved as usual.

## Acceptance

- F1: adding an invitee with a calendar draws that person's busy blocks and no titles.
- F2: adding an invitee with no shared calendar shows the label and leaves the slot pickable. ==**An empty calendar is never drawn as free time.**==

## Questions and assumptions

- Assumed: invitees outside the organization share no calendar by default.

## Out of scope

Recurring meetings.

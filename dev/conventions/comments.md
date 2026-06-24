# Comment Conventions

## Write only what code cannot describe

Add comments only to supplement information that is not obvious from the source code like:

- The reason or background of implementation (why / why not)
- Reference links
- Short summary of complex code block

## Keep comments local to what they annotate

A comment should be understandable and verifiable from the code unit it lives in
(this function, type, or operation) — not from knowledge that belongs to another
context. Explain the code in its own terms.

Avoid mixing in context the reader of this unit should not need, such as:

- **Caller behavior** — how some caller happens to use this code today.
- **Reachability arguments** — claims that a branch is "unreachable in practice"
  because of how the rest of the system currently behaves.
- **Unrelated subsystems** — naming another module's internals to justify a local
  decision.
- **Change history** — what the code "used to do" or replaced. Git history covers this.

Such comments rot when the other context changes, and they leak concerns across
boundaries that the abstraction is meant to keep separate. Justify a local
decision with a local, intrinsic reason instead.

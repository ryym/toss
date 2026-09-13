# Goal

Support readline-style (emacs mode) key bindings in the search input prompt:

- `Ctrl-f` / `Ctrl-b`: move the cursor right / left
- `Ctrl-a` / `Ctrl-e`: move the cursor to the start / end of the input
- `Ctrl-k`: delete from the cursor to the end of the input
- `Ctrl-h`: delete the character before the cursor (same as Backspace, including cancelling the search when the input is empty)
- `Ctrl-g` / `Ctrl-c`: cancel the search input (same as Esc)

# Context

The search prompt currently handles only the bare minimum keys: printable characters, Backspace, Left/Right, Enter and Esc.
Users who are used to readline-like line editing expect the common emacs-style bindings to work there.

The key handler also ignores modifiers when inserting characters, so pressing e.g. `Ctrl-f` today inserts a literal `f` into the query.
Handling Ctrl-modified keys explicitly fixes that as well: unbound Ctrl keys should do nothing instead of inserting a character.

`Ctrl-c` quits the pager in view mode, but in search input mode it only cancels the search.
This matches the usual shell/readline feel where `Ctrl-c` aborts the current input first; pressing it again in view mode quits.

Out of scope: other readline bindings such as `Ctrl-d`, `Ctrl-u`, `Ctrl-w`, a kill ring with `Ctrl-y`, and Home/End keys.

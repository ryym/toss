---
type: development
tags: [search, renderer]
---

## Goal

Make the current match distinguishable from the others even when the match is only one
character long.

## Context

**At a match length of one character, the difference between the current match and the rest
collapses into the presence or absence of an underline.**

The current highlighting scheme (`src/renderer/highlight.rs`):

| Target                            | Style                      |
| --------------------------------- | -------------------------- |
| Current match, whole              | reverse + bold             |
| Non-current match, first char     | reverse + underline + bold |
| Non-current match, rest           | underline + bold           |

- What the scheme aims for
  - No dependence on color
  - Every match stands out clearly
  - The current match is identifiable
- The third one breaks down for a single-character match
  - Current: reverse + bold
  - Non-current: reverse + underline + bold
  - The only difference is the underline, which is hard to tell apart

Notes:

- **The criterion is "the match is one character", not "the query is one character"**
  - With regex search, match lengths can be mixed on one screen, as in `a|bc`
  - Any fix has to be reasoned about per match
- **This is not critical**
  - `less` does not distinguish the current match visually at all
  - Single-character searches are themselves rare. Deciding not to act is defensible
- **Options that introduce color break an existing premise**
  - Toss currently uses no color of its own (SGR 30-49)
  - Everything is built from attributes (reverse/bold/underline)

## Options

### Option A: Drop reverse from single-character non-current matches

Render a non-current match that is one character long with underline + bold only, without
reverse. The rule "reverse extends across the whole match = current" then holds regardless of
length.

**Pros**

- A small change contained within `highlight.rs`. The three existing styles are reused as they are
- The signal identifying the current match (reverse) becomes consistent across all match lengths
- The appearance of multi-character matches does not change at all

**Cons**

- Flicker during incremental search
  - Every non-current match changes appearance the instant `f` becomes `fo`
- With underline alone, non-current matches do not stand out
- When regex search mixes match lengths, single-character matches look plainer than the rest
  - This introduces a different inconsistency: matches no longer stand out uniformly
- Adds a special case to the code and the tests: "the rule changes only at length 1"

### Option B: Give the current match a color

Render the current match with reverse plus a specific background/foreground color, so it is
distinguished by color regardless of length.

**Pros**

- Independent of match length and of neighboring characters, always identifiable at a glance.
  The most reliable option
- The current appearance of non-current matches does not have to change at all
- A widely used approach in search UIs, so it matches user expectations
  - vim/neovim keep `Search` for all matches and `CurSearch` for the current one as separate
    highlight groups
  - Browser find-in-page and various editors also color the current match differently

**Cons**

- Brings color into rendering that was built from attributes only
  - Reverse merely swaps the existing foreground/background, so it composes naturally with the
    colors of the input content
  - An explicit color breaks that, and can collide with the content's own foreground color and
    end up low-contrast
- Depends on the terminal color scheme
  - A hardcoded color is unreadable under some themes
  - Making it usable in practice requires making it configurable, which widens into the topic of
    a configuration mechanism (scope creep)
- Requires extending the style notation in `mock_screen.rs` and updating test expectations

### Option C: Signal the current match outside the match itself

Indicate the current match through a channel separate from the highlight. Two variants.

- **C-1** — Put a one-column marker (`>` or similar) at the left edge, pointing at the line that
  holds the current match
- **C-2** — Show a match number such as `3/12` in the status line

**Pros**

- Touches no character styling, so it does not break the existing appearance
- Completely independent of match length and regex
- The match counter in C-2 is useful information in its own right, independent of identifying the
  current match

**Cons**

- C-1 takes one column away from the content width, and every line shifts horizontally when a
  search starts, which moves the eye
- Both C-1 and C-2 only reveal which line
  - They do not resolve which match is current when one line holds several
  - Neither is a direct solution to the problem at hand
- C-2 makes the eye travel back and forth to the status line. Weak for grasping the position
  immediately

### Option D: Reserve reverse for the current match (simpler rule)

Render non-current matches entirely with underline + bold, and use reverse only for the current
match.

**Pros**

- The simplest possible rule: "reverse = current, underline = the other matches". Zero special
  cases by length
- None of the incremental-search flicker of Option A

**Cons**

- Non-current matches become distinctly less prominent
  - They get lost in content that already uses bold, or on terminals that render underline weakly
  - Tried it in practice: without reverse they really do become much harder to notice

### Option E: Do nothing (keep as is)

Accept "the current match is unidentifiable, but only for single-character matches" as a
tolerable limitation.

For reference, `less` has no visual distinction for the current match in the first place. In
`less`, however, the line holding the current match always comes to the top. Toss jumps with
`n`/`N` while scrolling the page as little as possible, the way vim does, so being unable to
identify the current match really is inconvenient.

**Pros**

- Leaves untouched an appearance that works fine for matches of two characters or more
- Sacrifices none of the permanent behavior, the code, or the scope
  - Every other option pays one of those for the relatively rare single-character match

**Cons**

- Walking through single-character matches with `n`/`N` loses the feedback of the operation
  - There is no way to tell how far you have moved
  - Since the eye cannot follow it, you press too many times or go back too far
- The design intent — "every match stands out, and the current one is identifiable" — stays
  quietly broken under a specific condition
  - It leaves a state that is awkward to explain as a specification

### Option F: Blink the current match (continuously)

Blink only the current match, distinguishing it regardless of length. There are two ways to
realize this.

- **F-1** — Use the terminal's SGR 5 (slow blink) directly
- **F-2** — Blink it ourselves, switching the style of the current match over time and
  redrawing

SGR 5 is implemented by xterm / the VTE family / iTerm2 / WezTerm / Windows Terminal / Konsole
and others, but it is commonly disabled by default or turned off by users, which makes it hard
to rely on. Adopting it would require checking on the target terminals in practice.

**Pros**

- Motion is far easier to detect than a static difference in attributes (the presence of an
  underline). Independent of match length
- F-2 redraws only the cells of the current match. The amount written is minimal
- F-2 does not depend on terminal support

**Cons**

- Constant blinking is fundamentally wrong for a pager
  - Part of a screen meant for sustained reading keeps blinking
  - F-2 is worse than native blinking because the user cannot turn it off in terminal settings.
    Adopting it would require an opt-out
- For accessibility, constant blinking is the kind of expression to avoid (WCAG guidance on
  flashing)
- F-1 cannot be assumed to appear
  - The identifying signal cannot rest on it alone, so a separate fallback appearance is needed
- F-2 can conflict with the renderer's "skip the redraw if the text is unchanged" optimization
  - In practice this is not a problem today, since lines with matches are always redrawn
- F-2 makes rendering time-dependent, which breaks the determinism of the MockScreen snapshot
  tests
  - A clock injection or a disable switch becomes permanently necessary for tests (see the
    Option G section)

### Option G: Flash the current match the moment it moves

Drop the constant blinking of F-2 and instead change the appearance only for an instant **at the
moment the current match moves to a different match**, then settle into the normal highlight.
Strictly speaking this is a single transition, not blinking.

Three degrees of freedom in the design.

- **Pulse count** — once (fade-like) or twice (more blink-like). More pulses are easier to
  notice but more irritating
- **The style during the flash** — momentarily "removing" the highlight (reverse off) is the
  lightest
  - It also reads visually as a blink. More natural than adding emphasis
- **Duration** — too short and it is missed, too long and it overlaps the next `n`

**Pros**

- Nearly all of Option F's drawbacks disappear
  - The timer terminates in one shot, so neither the fatigue of constant blinking nor the
    accessibility concern arises
- Directly solves the actual harm: "you cannot tell how far you have moved when hammering
  `n`/`N`"
- Like F-2, it rides on the event loop that already polls, and does not depend on terminal
  support

**Cons**

- **It solves nothing in the static state**
  - Useless for "scanning the screen to find which match is current"
  - Miss the instant it flashes and the cues are back to what they were
  - Unlike Options A/B/D, it is not a permanent means of identification
- **Suppression during incremental search is mandatory**
  - The current match moves on every typed character, so a naive implementation fires per
    keystroke and strobes the screen
  - The firing condition has to be separated out: only `n`/`N` after confirmation, and search
    submission
- Like F-2, it can conflict with the renderer's "skip the redraw if the text is unchanged"
  optimization
  - In practice this is not a problem today, since lines with matches are always redrawn

#### What the Option G PoC showed

A minimal form — one pulse, with the flash simply removing the reverse — was implemented and
checked on a real terminal on the branch `poc/single-char-match-highlight/flash`. The findings
follow.

**The right duration is far shorter than expected. Around 24ms was just right in the PoC.** The
impression on real hardware was that "even a short flash is perfectly recognizable". There are
two side effects, though.

- **The held-down `n` problem nearly disappears**
  - At the initial 175ms, the flash restarts on every jump
  - With fast key repeat, reverse stays off for the whole burst, which reads as "dim" rather than
    "blinking"
  - At 24ms it is well below the repeat interval, so each step is perceived as its own pulse
- **In exchange, the polling interval stops being negligible**
  - The flash lasts only about one or two frames of the refresh rate
  - The granularity of expiry detection (`FRAME_DURATION_FLASHING`) grows large relative to the
    flash duration
  - A 10ms granularity is a 40% error against a 24ms flash, which can surface as variation
    between one frame and two
  - This is the regime where the terminal's own refresh rate also acts as a quantization floor

**No interference with the scroll animation today.**
`n`/`N` call `scroll_physics.stop()` and jump in a single step. Only `d/u/f/b` animate, and they
do not move the current match. The concern returns only if jumping between matches is ever
animated.

**Strobe suppression needs no diffing.** Rather than comparing "did the current match change?",
firing directly from the key handlers for `n`/`N` and search submission avoids touching the
incremental path (`update_search_query`) at all. The keystroke strobe can be made **structurally
impossible**.

**Time-dependent rendering breaks the determinism of the MockScreen snapshot tests.**

- Two ways the PoC copes
  - Existing tests disable the flash (`MatchFlash::disabled()`), keeping rendering identical to
    before the feature
  - Only the new test uses `Duration::ZERO` to create a "flash lasting exactly one render" and
    verify it deterministically
- But all that verifies is the state transitions
  - **Both the duration and the polling granularity are entirely outside test coverage**
  - Exactly the parts that need tuning on real hardware, as described above, drop out
- Furthermore, since every existing test has the flash disabled, **the flash path is never
  exercised by any other test**
- Adopting this for real calls for injecting a clock (passing an `Instant` into `tick()`,
  interposing a `Clock` trait, etc.)
  - The actual duration can then be verified deterministically, and the flash can stay enabled in
    tests

## Assessment at the time of writing

**From what has actually been tried, Option G is the best.**

- If the position is identifiable at the `n`/`N` jump, it does not matter much that it becomes
  hard to distinguish afterward
- Even for matches of two characters or more, the flash on jump only makes the position easier to
  see; it does not get in the way
- Color can continue to be avoided

**The color specification of Option B is something to support eventually, but not to require.**

- There is always a possibility of interference with the colors of the text being displayed
- Once a UI that is comfortable without relying on color is secured, this can be considered later
  as an additional improvement

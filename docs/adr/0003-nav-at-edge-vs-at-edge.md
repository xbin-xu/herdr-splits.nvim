# `nav_at_edge` decides the pane boundary, `at_edge` the editor boundary

At a chord that reaches the editor's outer window *and* the pane boundary at the same time, two policies used to be coupled: the "is there a pane on the opposite side to cross into" test lived *inside* the `at_edge = 'wrap'` branch. So `at_edge='stop'` halted before `nav_at_edge` was ever consulted (pressing `<C-k>` in a full-height nvim pane could not reach the pane below), and `at_edge='wrap'` had to be turned on to get any pane flip at all — at the cost of also wrapping inside the editor (`<C-l>` in the rightmost split jumped to the leftmost one, which neither plain Vim, vim+tmux nor nvim+tmux do).

The pane flip is now decided first, and only for the two `at_edge` modes that describe "there is nowhere left to go" (`stop` and `wrap`). Whatever the flip does not consume falls through to `at_edge`. `at_edge='split'` and a custom `at_edge` function stay editor-level intents: they always win and never cross a pane.

## Considered options

- **Leave the coupling, document it.** Cheapest, but it makes "wrap across panes, stop inside the editor" — the combination that matches tmux and plain Vim — inexpressible; a user has to choose between `<C-k>` reaching the pane below and `<C-l>` not jumping across the screen.
- **Expose the flip to the `at_edge` function only** (`ctx.cross_pane()`), leaving defaults alone. No behaviour change, but the common configuration would still require writing a Lua function to get a tmux-like feel.
- **Decide the flip first, unconditionally.** Simplest rule, but it would also hijack `at_edge='split'`: at the outer edge with a pane behind, the user asked for a new split and would get a focus change instead.
- **Chosen:** flip first for `stop`/`wrap`, `split`/function untouched.

## Consequences

- `at_edge='stop'` + `nav_at_edge='wrap'` now crosses into the opposite-side pane when there is one, and halts when there is not. Anyone relying on "stop halts regardless of `nav_at_edge`" (documented before this change) gets one extra hop in layouts that have a pane on the opposite side.
- A chord that ends up halting now costs two extra `pane.edges`-style RPCs (the flip test) before it stops. Measured as negligible next to the keypress roundtrip, and the same calls the `wrap` path already made.
- README and CONTEXT.md state the division of labour explicitly, and `tests/test_nav.lua` pins all four combinations (stop/wrap crossing or halting, split/function never crossing).

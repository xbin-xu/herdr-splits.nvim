# herdr-splits

Directional movement (and, in Neovim, resizing) shared between an editor's own splits and the Herdr panes around it. The goal is one keymap with no visible seam: the user should not have to know whether the editor or the multiplexer is handling the key.

## Language

### Layout

**Herdr pane**:
A rectangle in Herdr's layout that owns a pty running an editor or a shell.
_Avoid_: window, split, terminal

**Editor window**:
One split inside a single editor instance — a Neovim window or a Vim window. Many editor windows live inside one Herdr pane.
_Avoid_: pane, split

**Zoom**:
Temporary promotion of one Herdr pane to the whole tab area.
_Avoid_: fullscreen, maximize

### Movement

**Chord**:
The key combination the user presses (`<C-h>`, `<C-j>`, `<C-k>`, `<C-l>`, `<M-h>` …). A chord is not a mapping; the mapping is whatever handles that chord in a given mode.
_Avoid_: keybind, keymap, shortcut

**Hand-off**:
The moment a chord stops moving inside the editor and asks Herdr to move the focus to a sibling pane.
_Avoid_: delegation, forwarding

**Edge**:
In one direction, the *editor edge* is where no further editor window exists, and the *pane edge* is where no further Herdr pane exists. A chord can be at either, both, or neither.
_Avoid_: boundary, border

**Edge behaviour**:
What a chord does once it is at both edges (`at_edge` for the editor side, `nav_at_edge` for the pane side): wrap, stop, or split.
_Avoid_: wrap mode, corner case

**Pane flip**:
Continuing on the opposite side of the layout, into the pane there, when a chord reaches the pane boundary — decided by `nav_at_edge` alone, before `at_edge` gets a say.
_Avoid_: cross-wrap, edge wrap

**Most recently focused**:
Choosing among candidates that are equally adjacent in a direction by focus recency instead of by cursor geometry — tmux's own rule.
_Avoid_: MRU list, last used

### Terminals

**Terminal-Job mode**:
The state of a terminal window in which keystrokes belong to the program running in it.
_Avoid_: insert mode, live mode

**Terminal-Normal mode**:
The state of a terminal window in which its scrollback is browsed like a buffer — cursor, gutters, line numbers.
_Avoid_: normal mode, scrollback mode

**Borrow**:
Running one Ex command from Terminal-Job mode through the `CTRL-W :` prefix and returning to Terminal-Job mode, the way `<C-o>` borrows Normal mode from Insert mode. A borrow is not a mode change.
_Avoid_: escape, exit, mode switch

**Skip filetype**:
A `filetype` whose terminal buffers keep the chord for the program inside, because that program already uses it (fzf moves its list with `CTRL-J`/`CTRL-K`; a nested Vim and lazygit want theirs too).
_Avoid_: blacklist, ignore list

### Configuration

**Instance policy**:
The navigation settings that belong to a Herdr instance rather than to one editor — the forwarded chords, `unzoom_on_nav`, `nav_at_edge`, `most_recently_focused` — read by the helper, plain Vim and Neovim alike.
_Avoid_: settings, managed config, global config

**`herdr-splits.conf`**:
The file the instance policy lives in, created by the helper when it is missing and never written by an editor unless explicitly asked. Its scaffold spells out the built-in defaults, so creating or deleting it changes nothing behaviourally.
_Avoid_: generated config, the conf file, herdr config

**Shipped defaults**:
The values every participant falls back to when the conf is missing — what the scaffold writes, so a missing conf and a fresh one agree.
_Avoid_: built-in config, hardcoded defaults

**Marked conf**:
A `herdr-splits.conf` whose first line is the generated marker, written by `setup({ overwrite_conf = true })` — a projection of the Lua opts, regenerated on every start.
_Avoid_: generated conf, auto config

**Unmarked conf**:
A `herdr-splits.conf` without the generated marker — the human's file; only `overwrite_conf` mode ever replaces it.
_Avoid_: hand-written conf, manual config

**Editor override**:
A managed setting given to one editor (`setup()` opts in Neovim, `g:herdr_splits_*` in Vim) that applies to that editor's own panes instead of the instance policy.
_Avoid_: local config, per-editor setting

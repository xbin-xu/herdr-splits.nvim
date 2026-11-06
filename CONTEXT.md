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

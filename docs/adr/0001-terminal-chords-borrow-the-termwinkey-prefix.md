# Terminal chords borrow `CTRL-W :` instead of leaving Terminal-Job mode

A terminal-mode chord needs to run an Ex command (`herdr_splits#nav`), and the obvious way to allow that — prefixing the mapping with `<C-\><C-n>` — is destructive: it parks the window in Terminal-Normal mode, so the moment the cursor moves elsewhere Vim paints the shell scrollback as a buffer (line numbers, cursor in the output, no more live shell) purely as a side effect of pressing `<C-k>`. The mapping now expands to `<C-W>:call herdr_splits#nav('k')<CR>`, which runs one Ex command and returns to Terminal-Job mode (`:h t_CTRL-W_:`), the way `<C-o>` borrows Normal mode from Insert mode. This is the same mechanism vim-tmux-navigator uses, which is why the tmux setup never showed the symptom.

## Considered options

- **Restore Job mode after the move** — `win_execute(src_winid, 'normal! i')` on the window the chord came from. Works (measured), but keeps a mode switch as an intermediate state, has to remember the source window, and flickers the scrollback for a frame.
- **Hide the symptom** — `autocmd TermOpen * setlocal nonumber norelativenumber`. The line numbers disappear; the scrollback still replaces the live shell, and `i` is still required to type again.
- **Borrow the prefix (chosen)** — no mode change ever happens, so there is nothing to restore, and the chord behaves identically whether or not the pane was just repainted (a zoomed pane resizing its pty was what made the leftover state visible in the first place).

## Consequences

The chord now depends on `'termwinkey'` (CTRL-W by default). The plugin reads `'termwinkey'` when the key fires, so rebinding the prefix to another single key keeps working; a rebind that would also need different *mappings* still has to be mapped by hand. vim-tmux-navigator carries the same constraint.

<div align='center'>

# herdr-splits.nvim

<p align="center">
    <em>Check out my other Herdr-related projects.</em> <br>
    <a href="https://github.com/lmilojevicc/seshagy"><em>Seshagy - Agent aware TUI session manager</em></a>
    &nbsp; | &nbsp;
    <a href="https://github.com/lmilojevicc/herdr-tab-rename"><em>Herdr tab rename - Autorename tabs for Herdr</em></a>
</p>

</div>

Seamless navigation between Neovim/Vim splits and [Herdr](https://herdr.dev) panes, plus native resizing for Neovim splits. Makes Herdr terminal splits behave like native editor windows — move (and, in Neovim, resize) as if they were all part of the same editor.

Inspired by [smart-splits.nvim](https://github.com/mrjones2014/smart-splits.nvim), ported to Herdr's CLI.

## Features

- **Seamless navigation**: Same keys work in Neovim, plain Vim, and plain shells — Herdr forwards them to the editor when appropriate, the editor delegates to Herdr at edges.
- **Seamless resizing** (Neovim): `<M-h/j/k/l>` resizes Neovim splits natively, delegates to Herdr when a window fills the terminal. Plain Vim keeps pane-level resizing.
- **Plain Vim support**: `Ctrl+hjkl` navigation (normal and terminal mode) with the same MRU/config semantics as Neovim, including at the pane edge.
- **at_edge behaviours**: `wrap` (default), `stop`, `split`, or a custom function.
- **Plugin-aware**: Ignores snacks/neo-tree/dadbod-ui/aerial sidebars and embedded floats (zindex < 50) by default — your keybinds never get trapped inside a picker.
- **Zoom-aware navigation**: leaves a zoomed pane zoomed by default (the zoom follows the focus); `unzoom_on_nav=true` reveals the destination pane instead. Never unzooms when moving between Neovim splits in one pane.
- **Wrap-around** (opt-in via `nav_at_edge=wrap`): at a layout edge, navigation continues on the opposite side — crossing into a sibling Herdr pane when one exists (e.g. `pane | nvim` → `ctrl+l` wraps to the pane), otherwise wrapping within Neovim. Herdr-pane navigation wraps around at edges too (past the last pane → the first).
- **Count prefix support**: `3<C-h>` moves three splits left; `5<M-l>` resizes five steps right.

## Requirements

- Neovim ≥ 0.10 for the Lua plugin, **or** Vim ≥ 8.1 with `+job` for
  plain-Vim navigation
- [Herdr](https://herdr.dev) ≥ 0.8.0 (for plugin actions)
- **Rust to build the Herdr side.** A `[[build]]` hook compiles `rust/` into
  `scripts/bin/herdr-splits` during `herdr plugin install`. The crate has no
  dependencies, so the build needs no network access once the toolchain is
  installed. For a linked checkout, run `scripts/build.sh` /
  `scripts\build.ps1` once (see [Local development](#local-development)).
  The helper is what keeps a keypress at ~10 ms; the `python -S` start it
  replaced cost ~50 ms on Windows, more than the RPC work itself.

The Python scripts (`scripts/herdr-*.py`) and bash scripts (`scripts/herdr-*.sh`)
stay in the tree as the reference implementation and the no-toolchain fallback;
the Herdr actions no longer invoke them.

## Installation

Three steps — install on both the Herdr side and the Neovim side, then add keybinds.

### 1. Install the Herdr plugin

This provides the `nav-*` and `resize-*` actions that Herdr keybinds invoke.

```bash
# From GitHub (once published). Runs the [[build]] commands, which compile the
# Rust helper and install it as scripts/bin/herdr-splits:
herdr plugin install lmilojevicc/herdr-splits.nvim

# Local development (build commands do NOT run on link — build it yourself):
herdr plugin link /path/to/herdr-splits
/path/to/herdr-splits/scripts/build.sh
```

### 2. Install the Neovim plugin

## lazy.nvim

```lua
{
  'lmilojevicc/herdr-splits.nvim',
  -- For local development, swap the repo line for `dir = '/path/to/herdr-splits'`
  -- (see "Local development" below).
  cond = vim.env.HERDR_ENV == '1',
  event = 'VeryLazy',
  -- Optional: auto-sync the Herdr-side scripts when lazy updates this plugin.
  -- Requires `auto_sync_herdr = true` in setup() below to take effect.
  -- build = ':lua require("herdr-splits").sync_herdr()',
  config = function()
    require('herdr-splits').setup({
      -- Defaults shown. All fields optional.
      default_amount = 0.03,       -- Herdr resize ratio
      neovim_amount = 3,           -- Neovim resize cells
      at_edge = 'wrap',            -- 'wrap' | 'stop' | 'split' | function
      ignored_buftypes = { 'nofile', 'quickfix', 'prompt', 'help', 'terminal' },
      ignored_filetypes = {
        'NvimTree',
        -- sidebars
        'neo-tree',
        'snacks_dashboard',
        'snacks_explorer',
        'snacks_picker',
        -- DB / REPL / data sidebars
        'dadbod-ui',
        'dbout',
        -- outlines / symbols
        'aerial',
        'Outline',
        -- diagnostics / quick lists
        'Trouble',
        'quickfix',
      },
      move_cursor_same_row = false,
      herdr_bin = nil,                -- auto-detected from HERDR_BIN_PATH
      floating_zindex_max = 50,       -- floats with zindex < this are treated as embedded sidebars
      ignore_previewwindows = false,  -- opt-in: also treat previewwindow windows (e.g. .dbout) as sidebars
      -- auto_sync_herdr = true,      -- opt-in: sync Herdr-side scripts on update
      --
      -- The instance policy (unzoom_on_nav, nav_at_edge, most_recently_focused and
      -- the forwarded chords) lives in herdr-splits.conf, which Herdr, plain Vim
      -- and Neovim all read; the helper creates it with the defaults on first use.
      -- Set these only to override them for THIS editor's panes:
      -- unzoom_on_nav = false, nav_at_edge = 'wrap', most_recently_focused = true,
      -- Opt-in: make setup() the authority instead: it then writes the policy to
      -- the conf and ignores what an earlier setup() put there.
      -- overwrite_conf = true,
      --
      -- Opt-in: re-map the nav chords in terminal buffers so the edge hands
      -- off to Herdr instead of stopping inside Neovim (see "Terminal-mode
      -- navigation" below). `true` = every pty terminal.
      terminal_nav = { filetypes = { 'snacks_terminal' } },
    })
  end,
  keys = {
    { '<C-h>', function() require('herdr-splits').move_cursor_left() end,  desc = 'Navigate left' },
    { '<C-j>', function() require('herdr-splits').move_cursor_down() end,  desc = 'Navigate down' },
    { '<C-k>', function() require('herdr-splits').move_cursor_up() end,    desc = 'Navigate up' },
    { '<C-l>', function() require('herdr-splits').move_cursor_right() end, desc = 'Navigate right' },
    { '<M-h>', function() require('herdr-splits').resize_left() end,  desc = 'Resize left' },
    { '<M-j>', function() require('herdr-splits').resize_down() end,  desc = 'Resize down' },
    { '<M-k>', function() require('herdr-splits').resize_up() end,    desc = 'Resize up' },
    { '<M-l>', function() require('herdr-splits').resize_right() end, desc = 'Resize right' },
  },
}
```

## packer.nvim

```lua
use {
  'lmilojevicc/herdr-splits.nvim',
  config = function()
    require('herdr-splits').setup()
  end,
}
```

### 2b. Plain Vim (navigation only)

Add the checkout to `runtimepath` with your plugin manager, e.g. vim-plug:

```vim
Plug 'lmilojevicc/herdr-splits.nvim'
" while testing a fork/branch:
" Plug 'xbin-xu/herdr-splits.nvim', { 'branch': 'feat/windows-support' }
```

The Vim runtime files are `plugin/herdr-splits.vim` (title advertisement and
`C-hjkl` mappings) and `autoload/herdr_splits.vim` (navigation, MRU, edge
handoff).

Inside a Herdr pane the plugin:

- maps `<C-h/j/k/l>` in normal and terminal mode;
- reads `most_recently_focused`, `unzoom_on_nav`, `nav_at_edge` and the
  forwarded chords from `herdr-splits.conf` (`herdr-splits init` writes a
  scaffold; the file is shared with Neovim and the Herdr-side helper, so both
  editors stay in step — override per editor with
  `let g:herdr_splits_most_recently_focused = 0` or `1` if you really need to);
- hands the direction to `scripts/bin/herdr-splits nav --pane-only` (the same
  Rust helper the Herdr actions run) when Vim is at its edge. The hand-off is
  synchronous — the helper lives for milliseconds, and a chord should not race a
  focus change that has not happened yet — and the argv list goes straight to
  the process, so no shell, no quoting, no `system()` string pitfalls on
  Windows. If the helper has not been built, it falls back to
  `python -S scripts/herdr-nav.py --pane-only`.

Configuration (set before the plugin loads):

```vim
let g:herdr_splits_bin = ''                   " explicit helper path (default: this checkout's build)
let g:herdr_splits_python = 'python'          " fallback interpreter, only if the helper is missing
let g:herdr_splits_script = ''                " fallback herdr-nav.py path
let g:herdr_splits_most_recently_focused = 1  " override the instance policy for Vim
let g:herdr_splits_terminal_nav = 1           " terminal-mode chords (install-time opt-out)
let g:herdr_splits_terminal_skip_filetypes = ['fzf']  " terminal programs that keep C-hjkl
```

If you also use vim-tmux-navigator, disable its mappings in the vimrc (before
any plugin loads); this plugin owns `C-hjkl` inside Herdr:

```vim
if $HERDR_ENV ==# '1'
  let g:tmux_navigator_no_mappings = 1
endif
```

Notes:

- Mappings are defined only when `$HERDR_ENV == '1'`, so tmux navigation
  outside Herdr keeps working.
- Plain Vim has navigation only. If you bind the Herdr `resize-*` actions,
  they resize the **Herdr pane** when the focused pane is plain Vim (Neovim
  handles that chord itself).
- On Windows, Herdr's `pane process-info` cannot see programs started from the
  pane shell. The plugin therefore appends `[herdr-splits]` to `titlestring`
  (only inside Herdr) so detection identifies the plugin, not merely "some
  Vim". Do not `set notitle` after startup, and start Vim with the plugin
  loaded.
- Neovim-only parity gaps in plain Vim: no local `at_edge=wrap`/`split`
  behavior, no float/sidebar handling, and a count prefix stays inside Vim
  (it never crosses a pane boundary).
- Plain Vim maps the chords in **both** normal and terminal mode (the terminal
  `tnoremap` is the only way to leave a terminal split there).
- Plain Vim can override the policy for its own edge hand-off, since it reads
  the instance policy rather than owning it (`autoload/herdr_splits.vim` passes
  the overrides to the helper as `--nav-at-edge` / `--unzoom`):
  `let g:herdr_splits_nav_at_edge = 'stop'` and
  `let g:herdr_splits_unzoom_on_nav = 0`. Leave them unset and the conf decides
  — which is what you normally want, so both editors agree.
  `g:herdr_splits_most_recently_focused` is forwarded too, so the pane-level
  pick agrees with the in-window one.

  In terminal mode the chord **borrows Vim's `CTRL-W` prefix** instead of
  leaving Terminal-Job mode: it expands to `<C-W>:call herdr_splits#nav('k')<CR>`
  (`:h t_CTRL-W_:`), the way `<C-o>` borrows Normal mode from Insert mode. The
  terminal window you pressed it in therefore stays in Terminal-Job mode, so the
  shell is never repainted as a buffer with line numbers over it. That is the
  same mechanism vim-tmux-navigator uses — its t-mode maps also start with
  `<C-W>:` — which is why navigating in tmux never showed the scrollback. The
  prefix follows `'termwinkey'`, so a non-default value keeps working;
  `<C-\><C-n>` stays *your* gesture for looking at the scrollback.

  Entering a terminal window from elsewhere is left to Vim, which keeps such a
  window in Terminal-Normal mode until `i`: the chord does not force Job mode on
  a window you deliberately parked.

  Some terminal programs own `C-hjkl` themselves — fzf moves its list with
  `CTRL-J`/`CTRL-K`, a nested Vim wants them, lazygit takes some — so their
  buffers keep the keys and the chord is handed to the program verbatim
  (exact `filetype` match, empty list = map every terminal):

  ```vim
  let g:herdr_splits_terminal_skip_filetypes = ['fzf']   " the default
  ```

  Neovim's equivalent is opt-in and allow-listed (`terminal_nav`, see 2c)
  because there it only *replaces* interception a distribution already does.
  Opt out of the Vim t-mode maps entirely with
  `let g:herdr_splits_terminal_nav = 0` (install-time: normal mode keeps working,
  no t-mode map is defined). To keep the chords for a single terminal buffer or
  a terminal plugin, win the mapping yourself — buffer-local beats global, and
  the plugin then never shadows it:

  ```vim
  autocmd TerminalOpen * tnoremap <buffer> <C-h> <C-h>
  ```

### 2c. Terminal-mode navigation (Neovim)

Plain Vim has always crossed panes from inside a terminal (`tnoremap` in
`plugin/herdr-splits.vim`). Neovim needs the same thing **buffer-locally**, and
that is what `terminal_nav` does:

```lua
require('herdr-splits').setup({
  -- true = every pty terminal; a table scopes it
  terminal_nav = { filetypes = { 'snacks_terminal' } },
})
```

Why it cannot be a plain global mapping: editor distributions and terminal
plugins map the chords **in the terminal buffer**, and Neovim resolves
buffer-local before global. LazyVim/snacks, for example, install

```lua
nav_j = { '<C-j>', function() vim.schedule(function() vim.cmd.wincmd('j') end) end,
          expr = true, mode = 't' }
```

in `lua/lazyvim/plugins/util.lua`. `wincmd` only moves Neovim windows: at the
Neovim edge it fails silently, so the chord never reaches Herdr and the cursor
is stuck in the terminal split. The plugin therefore maps one tick after
`TermOpen` (which fires after such a plugin has mapped its window) and wins.

Behavior:

- **Tiled terminals** (a terminal split): the chord routes through
  `move_cursor_*`, so it moves between Neovim windows, crosses into the sibling
  Herdr pane at the edge, and `at_edge`/`nav_at_edge` decide what happens when
  both edges are reached (default `wrap`).
- **Floating terminals** keep the chord: it belongs to the program inside
  (htop, fzf, ssh, ...). The mapping returns the key unremapped, mirroring
  LazyVim's own `is_floating` branch.
- The maps follow the chords in the conf, so a remap (`nav_key_left=alt+left`)
  applies in terminal mode too — `nav_keys_nvim` is derived from the conf.
- Scope it: `terminal_nav = true` covers every `buftype=terminal` buffer
  (`:terminal`, toggleterm, dap-repl-style ptys, ...). If a terminal runs
  something that wants the chords itself (a nested Vim, htop), either narrow it
  with `filetypes = {...}` (allow-list) or exclude it with
  `skip_filetypes = {...}`. A plain `:terminal` has an empty `filetype`, so
  prefer the allow-list when you only want your terminal plugin covered.
- Buffers that are not `buftype=terminal` — including `dap-repl` (a `prompt`
  buffer) — are never touched.
- Session restore rebuilds terminal buffers without firing `TermOpen`, so
  `setup()` also maps the terminal buffers that already exist.

Runtime re-apply after changing the config: `require('herdr-splits').setup_terminal_nav()`.

### 3. Add keybinds to Herdr config

Add these to `~/.config/herdr/config.toml`, then run `herdr server reload-config`:

```toml
[[keys.command]]
key = "ctrl+h"
type = "plugin_action"
command = "herdr-splits.nav-left"

[[keys.command]]
key = "ctrl+j"
type = "plugin_action"
command = "herdr-splits.nav-down"

[[keys.command]]
key = "ctrl+k"
type = "plugin_action"
command = "herdr-splits.nav-up"

[[keys.command]]
key = "ctrl+l"
type = "plugin_action"
command = "herdr-splits.nav-right"

[[keys.command]]
key = "alt+h"
type = "plugin_action"
command = "herdr-splits.resize-left"

[[keys.command]]
key = "alt+j"
type = "plugin_action"
command = "herdr-splits.resize-down"

[[keys.command]]
key = "alt+k"
type = "plugin_action"
command = "herdr-splits.resize-up"

[[keys.command]]
key = "alt+l"
type = "plugin_action"
command = "herdr-splits.resize-right"
```

> **Note for macOS**: Terminals treat Option as a special character modifier by default. You need to set Option = Alt/Meta:
>
> - **Ghostty**: `macos-option-as-alt = true`
> - **Alacritty**: `option_as_alt = "Both"`
> - **kitty**: `macos_option_as_alt yes`
> - **iTerm**: Profiles → Keys → Left Option key → Esc+
>
> If you're using [smart-splits.nvim](https://github.com/mrjones2014/smart-splits.nvim) for tmux, add `cond = vim.env.HERDR_ENV ~= '1'` to its spec so the two plugins don't conflict.

### Instance policy: `herdr-splits.conf`

Five settings describe how navigation behaves for a whole Herdr instance, not
for one editor: the four forwarded chords (`nav_key_*`, `resize_key_*`),
`unzoom_on_nav`, `nav_at_edge` and `most_recently_focused`. They live in one
file that **every** participant reads — the Herdr-side helper (on every
keypress), plain Vim, and Neovim (in `setup()`, or `:HerdrSplitsReload`):

```
herdr plugin config-dir herdr-splits     # prints the directory
# e.g. ~/.config/herdr/plugins/config/herdr-splits/herdr-splits.conf
#      %APPDATA%\herdr\plugins\config\herdr-splits\herdr-splits.conf
```

`HERDR_SPLITS_CONFIG` overrides the file, `HERDR_PLUGIN_CONFIG_DIR` the
directory; both must be visible to the Herdr server *and* the editors.

**The helper creates it.** `nav`, `resize` and `install` write the scaffold when
it is missing, so a fresh install has a file to edit instead of only living
inside the defaults. No *editor* writes it as a side effect of starting up, so
the instance's policy is never decided behind your back. `herdr-splits init`
writes it explicitly (refuses to replace an existing file without `--force`,
`--print` writes to stdout):

```sh
herdr-splits init            # write it now, with a message
herdr-splits init --print    # just show the scaffold
herdr-splits init --force     # reset it to the defaults
herdr-splits doctor          # what is in effect, and where it came from
```

**A missing file is a valid state.** Every reader then falls back to the same
built-in defaults the scaffold spells out — and the scaffold *is* those
defaults, so creating it changes no behaviour. Deleting the file is therefore a
reset: the helper recreates it with the defaults below.

**Format.** One `key=value` per line, `#` starts a comment, and the *last*
occurrence of a key wins (so a generated file stays readable):

```
nav_key_left=ctrl+h
resize_key_left=alt+h
unzoom_on_nav=false
nav_at_edge=wrap
most_recently_focused=true
```

The shipped defaults are `unzoom_on_nav=false` (the pane stays zoomed; the zoom
follows the focus), `nav_at_edge=wrap`, `most_recently_focused=true` and
`ctrl+hjkl` / `alt+hjkl`.

**Neovim opts vs the file.** `setup()` reads the file and then applies its own
managed opts (if any) *for its own panes only* — an escape hatch, not a way to
configure the instance. Setting them in one editor while another reads the file
is exactly how the two drift apart, so prefer the file alone.

**`overwrite_conf = true`** flips that: `setup()` becomes the authority, writes
the file on every start (stamping `# herdr-splits-generated-v1` as its first
line) and ignores what a previous `setup()` generated, so removing an opt
reverts it. `:HerdrSplitsReload` re-projects and rewrites the file too, so the
helper cannot end up reading a value this session no longer uses. Use it when
you want Lua to be the single source; leave it off (the default) when the file
is.

**Editing rules.** With `overwrite_conf` off (the default) nothing ever rewrites
the conf: every value you write is honoured, whichever form the file has.
*Editing* a file that carries the generated marker only sticks while
`overwrite_conf` stays off; with it on, `setup()` regenerates the file on every
start (that is what the marker means), so delete the
`# herdr-splits-generated-v1` line to take ownership instead.
`:checkhealth herdr-splits` reports which kind the current file is, plus the
values in effect.

**Freshness per reader.** The helper reads the conf per keypress, plain Vim per
chord, Neovim once per `setup()` — run `:HerdrSplitsReload` after editing the
file (a changed chord only reaches Terminal mode once those buffers are
reopened, because the maps are installed per buffer).

### Auto-unzoom and wrap-around

**Auto-unzoom** (off by default). With `unzoom_on_nav=true` in the instance
policy, navigating away from a **zoomed** pane unzooms first so the destination
is visible — whether you're crossing into a sibling Herdr pane or wrapping at a
layout edge. It never unzooms when moving between Neovim splits inside the same
pane.

The shipped default is `unzoom_on_nav=false`: **the pane stays zoomed and the
zoom follows the focus**, so pressing `ctrl+l` in a full-screen nvim lands you
full-screen in the neighbour.

**Wrap-around.** The shipped default is `nav_at_edge=wrap`: reaching the layout
edge continues on the opposite side. Set `nav_at_edge=stop` (see below) to halt
instead.

- If a Herdr pane sits on the opposite side, focus crosses into it — so
  `pane | nvim` wraps `ctrl+l` back to the pane, and
  `pane | nvim win1 | win2` wraps from win2 to the pane.
- Otherwise it wraps within Neovim (smart-splits default).
- Between plain Herdr panes, navigation wraps around at edges too (past the
  last pane → the first).
- Wrap works even when you're on a sidebar (dbui, neo-tree, quickfix, ...),
  so you can leave it at an edge. Parent-backed embedded floats use their
  containing split's geometry; unresolved embedded overlays remain gated.

**To disable auto-unzoom entirely**, set `unzoom_on_nav=false` in the instance
policy (see above):

```
unzoom_on_nav=false
```

**To stop at layout edges instead**, set `nav_at_edge=stop` in the instance policy:

```
nav_at_edge=stop
```

This single switch controls wrap-across-boundary on **both** sides:

- **Plain Herdr panes:** `wrap` (the default) wraps to the opposite pane at an
  edge; `stop` halts.
- **Neovim edge wrap:** when Neovim's `at_edge='wrap'`, `wrap` lets the wrap
  cross to the Herdr pane on the opposite side (e.g. `ctrl+l` from the last nvim
  split lands on the pane); `stop` keeps the wrap within Neovim (e.g. `ctrl+l`
  from the last split cycles to the first split instead of leaving Neovim).
  `at_edge='stop'` halts regardless of `nav_at_edge`.

Note: `at_edge` (Neovim window-edge behaviour: `wrap`/`stop`/`split`/function)
and `nav_at_edge` (Herdr pane-boundary wrap: `wrap`/`stop`) are two different
things despite the shared name — `at_edge` is Neovim-side only and is not
written to the conf.

Because `nav_at_edge`, `unzoom_on_nav`, `most_recently_focused` and the key
chords live in one conf **per Herdr instance**, the values apply to every pane
in that instance, including plain shells — and, since only an editor *reads*
them, two editors cannot overwrite each other's policy. (With
`overwrite_conf = true` the last Neovim to call `setup()` does win, which is
exactly the trade-off that option makes.)

Each participant can still override the policy for its own edge hand-off:

- **Plain Vim** forwards `g:herdr_splits_nav_at_edge`,
  `g:herdr_splits_unzoom_on_nav` and `g:herdr_splits_most_recently_focused` as
  `--nav-at-edge=wrap|stop`, `--unzoom=on|off` and
  `--most-recently-focused=on|off` on the helper call. Unset, the conf decides.
- **Neovim** decides in-process; the same three keys in `setup()` are its
  override, and it never writes the conf unless `overwrite_conf` is on.
- **Any caller** can pass the flags by hand — they override the conf for that
  one invocation (`herdr-splits nav --help`).

With `unzoom_on_nav=false` (the default), navigation leaves the pane zoomed.
Herdr keeps reporting the pre-zoom pane tiling and edge flags while zoomed, so
MRU selection and edge wrapping work as usual; focus moves and the zoom follows
it. With `unzoom_on_nav=true`, pressing toward an edge while zoomed first
unzooms the pane (so the destination is visible) and then halts — the pane
unzooms even though focus doesn't move.

**To remap the keys forwarded into Neovim**, edit `nav_key_*` / `resize_key_*`
in the instance policy. Override only the directions you rebind; the rest keep
their defaults. By default the helper forwards `ctrl+h/j/k/l` for navigation
and `alt+h/j/k/l` for resizing — the chords the documented Neovim keymaps bind:

```
nav_key_left=alt+left
nav_key_down=alt+down
nav_key_up=alt+up
nav_key_right=alt+right
```

- The conf uses Herdr chord notation (`alt+left`); `setup()` used to accept
  Neovim notation (`<M-Left>`) and translate it. `herdr pane send-keys` accepts
  the same notation the conf carries.
- Any direction left out keeps its default.
- The chord forwarded into Neovim must match the Neovim keymap that catches it,
  so keep your `vim.keymap.set` in sync. Terminal-mode maps do follow the conf
  automatically (`nav_keys_nvim` is derived from it).
- With `overwrite_conf = true` you may instead pass `nav_keys` / `resize_keys`
  to `setup()` in Neovim notation; without it they are ignored (with a warning)
  because Herdr has to know the chord and it reads the conf.

> **No eager load needed for custom chords.** The chords are in the conf, which
> the helper reads directly, so a lazy-loaded plugin is fine — Herdr can
> forward a chord before Neovim has loaded anything.

## Local development

Both sides load straight from your clone, so edits take effect immediately
(on the Herdr side; Neovim picks them up on reload/restart).

1. **Build the Herdr-side helper.** `herdr plugin link` does not run the
   manifest's `[[build]]` commands, so do it once by hand:

   ```bash
   # Linux / macOS
   ./scripts/build.sh

   # Windows
   pwsh -File scripts\build.ps1
   ```

   This is `cargo build --release` followed by
   `cargo run --release -- install`, which copies the binary to
   `scripts/bin/herdr-splits`. Re-run it after changing `rust/`.

2. **Link the Herdr plugin** to your local repo:

   ```bash
   herdr plugin link /path/to/herdr-splits
   ```

3. **Point lazy.nvim at the same path** — change the first line of the spec
   from the GitHub repo to `dir`:

   ```lua
   {
     dir = '/path/to/herdr-splits',
     cond = vim.env.HERDR_ENV == '1',
     -- ...rest of the spec unchanged
   }
   ```

Run `herdr plugin list` to confirm it shows `herdr-splits ... [local:/...]`.
To switch back to the published version: `herdr plugin unlink herdr-splits`,
then `herdr plugin install lmilojevicc/herdr-splits.nvim`, and revert the
lazy spec to `'lmilojevicc/herdr-splits.nvim'`.

Useful commands while iterating:

```bash
make build         # cargo build --release + install into scripts/bin/
make test-rust     # unit + end-to-end tests (no herdr server needed)
make check         # everything: shell, Lua, Vim, Python, Rust

scripts/bin/herdr-splits doctor    # resolved socket/conf/pane + RPC probe
```

`doctor` is the fastest way to see what a keypress would do: it prints the
resolved socket and conf paths, the current pane, the detected editor, and
whether the socket actually answers.

### Debugging the helper

Every RPC call can be recorded. This works without a running server too, using
a scripted fixture:

```bash
# Record what a real keypress does:
HERDR_PANE_ID=w1:p1 scripts/bin/herdr-splits nav left --trace /tmp/trace.log

# Replay a fixture instead of talking to herdr (what the tests do):
scripts/bin/herdr-splits nav left --replay tests/fixtures/nav-nvim.jsonl --trace /tmp/trace.log
```

Each trace line is `method<TAB>params-json<TAB>ok|error`.

## Lua API

```lua
require('herdr-splits').setup(opts)

-- Re-read herdr-splits.conf (instance policy) after editing it; also available
-- as :HerdrSplitsReload. Reports whether the policy or the chords changed.
require('herdr-splits').reload()

-- (Re)install terminal-mode navigation after changing `terminal_nav` at runtime
require('herdr-splits').setup_terminal_nav()

-- Resize (amount defaults to config.default_amount, multiplied by count prefix)
require('herdr-splits').resize_left(amount)
require('herdr-splits').resize_down(amount)
require('herdr-splits').resize_up(amount)
require('herdr-splits').resize_right(amount)

-- Navigate (opts can override same_row and at_edge per-call)
require('herdr-splits').move_cursor_left({ same_row = true, at_edge = 'stop' })
require('herdr-splits').move_cursor_down(opts)
require('herdr-splits').move_cursor_up(opts)
require('herdr-splits').move_cursor_right(opts)
```

## Compatibility

`herdr-splits.nvim` ships with an opinionated default `ignored_filetypes` list so your resize keybinds don't get trapped inside common sidebars and pickers. These filetypes are treated as sidebars: `split`-at-edge and Herdr resize delegation are skipped from them, and the flag is passed to a custom `at_edge` function as `is_sidebar`. `wrap`, however, still works from a sidebar so you can leave it (e.g. `ctrl+l` on a `dadbod-ui` drawer wraps to the opposite side). Parent-backed embedded floats use their containing split's geometry: inspection and resize preserve embedded-float focus, while successful navigation directly focuses the selected neighboring window without transiently focusing the layout parent. Unresolved embedded floats remain gated.

The default `ignored_filetypes` covers:

- `NvimTree` — `nvim-tree/nvim-tree.lua` default file tree (`buftype=nofile`, `winfixwidth`).
- `neo-tree` — `nvim-neo-tree/neo-tree.nvim` LazyVim default (`winfixwidth` sidebar).
- `snacks_dashboard` / `snacks_explorer` / `snacks_picker` — `folke/snacks` sidebar/picker windows (the float case is handled by `is_embedded_floating_window` below).
- `dadbod-ui` / `dbout` — `kristijanhusak/vim-dadbod-ui` drawer and `tpope/vim-dadbod` preview result buffer.
- `aerial` / `Outline` — code-outline sidebars.
- `Trouble` / `quickfix` — diagnostics and quickfix lists.

The default `ignored_buftypes` covers `nofile`, `quickfix`, `prompt`, `help`, and `terminal`.

**Embedded floating windows** (snacks explorer in float mode, neo-tree in float mode, aerial in float mode) are detected via the `zindex < floating_zindex_max` heuristic (default threshold 50, Neovim's default float zindex). When window-relative ancestry resolves to a normal window, geometry inspection and resize preserve embedded-float focus; successful navigation directly focuses the selected neighboring window without transiently focusing the layout parent. Otherwise the embedded float is gated. Override the threshold via the `floating_zindex_max` config field.

**Add your own at runtime** from any `VeryLazy` autocmd:

```lua
require('herdr-splits').add_ignored_filetype('lazy')
require('herdr-splits').add_ignored_buftype('help')
```

**Debug with `:checkhealth herdr-splits`** (Neovim ≥ 0.10) to see which filetypes/buftypes are ignored, whether Herdr is in session, and whether the current window is classified as a sidebar.

If you want to **completely replace** the default list (e.g. to opt out of one of the entries), note that `vim.tbl_deep_extend('force', M, opts)` **concatenates** the `ignored_filetypes` / `ignored_buftypes` tables you pass; you keep what you pass AND the defaults are appended. To opt out, assign directly after `setup()`:

```lua
require('herdr-splits').setup({ ignored_filetypes = {} })  -- inherit nothing
require('herdr-splits').add_ignored_filetype('mything')     -- then add back what you want
```

## How It Works

Two sides cooperate for seamless two-way navigation:

```text
You press C-h in a Herdr pane:

  ┌─ Herdr intercepts the key (plugin_action keybind)
  │
  ├─ scripts/bin/herdr-splits nav left   (Rust helper, ~10 ms)
  │
  ├─ Is the focused pane running an editor (Neovim / plain Vim with this plugin)?
  │   │
  │   ├─ YES → forward "ctrl+h" into that pane
  │   │         └─ the editor plugin receives it
  │   │              ├─ Has a window to the left? → wincmd h
  │   │              └─ At the editor edge? → cross into the Herdr pane
  │   │
  │   └─ NO (plain shell, etc.) → pane.focus_direction left
  │
  └─ Result: same keys move you everywhere
```

Both sides talk to Herdr's control socket directly — one NDJSON request per
connection, no `herdr` CLI subprocesses. Neovim does its edge crossing in-process
(`lua/herdr-splits/rpc.lua`); the Herdr-side actions and plain Vim's edge
hand-off use the same protocol from Rust (`rust/src/rpc.rs`). Pane-level
decisions (MRU, unzoom, wrap) live in one implementation per layer and share the
same `mru-panes` state file.

### Navigation

```text
1. Try moving within the editor (wincmd h/j/k/l)
   ├─ Window changed → done (stayed within the editor's splits)
   └─ Window didn't change → at the editor edge
        ├─ Zoomed? → with auto-unzoom on, unzoom first and retry;
        │             otherwise use Herdr's pre-zoom layout
        ├─ Check if Herdr is running (HERDR_ENV=1)
        ├─ Check if Herdr pane has a neighbour in this direction
        │   └─ Yes → herdr pane focus --direction → done
        └─ No (at both the editor AND Herdr edge) →
             apply at_edge behaviour (`wrap` crosses to the opposite Herdr
             pane if one exists and nav_at_edge=wrap (the default), else
             wraps within Neovim)
```

Plain Vim mirrors the local move (including the shared `most_recently_focused`
choice) and delegates at its edge to `herdr-nav.py --pane-only`. The
Neovim-only `at_edge` modes, float/sidebar handling, and local wrap do not
apply in plain Vim.

In terminal mode both editors keep the chord out of the job without changing
modes: plain Vim borrows `CTRL-W :` (see 2b), Neovim never leaves Terminal-Job
mode because its `terminal_nav` mapping is an `<expr>` map that returns nothing
and schedules the move. That is why the two behave identically on the "line
numbers over the shell" front, and why nothing here depends on how often the
terminal is repainted.

### Resizing

```text
1. Only one Neovim window in this dimension AND fills terminal?
   └─ Yes → pane.resize <direction> <amount> → done
2. Otherwise:
   └─ Native Neovim resize with position-aware +/- operators
```

Plain Vim is treated like a plain pane for resizing: the `resize-*` actions
resize the Herdr pane directly, because the Vim adapter implements navigation
but no resize mappings.

### Herdr-side helper

`scripts/bin/herdr-splits` is the only thing a keypress runs on the Herdr side
(and what plain Vim spawns at its edge). It is a dependency-free Rust binary:

| Subcommand | What it does |
| --- | --- |
| `nav <dir> [--pane-only]` | the navigation decision tree, then `pane.send_keys`, `pane.focus` or `pane.focus_direction` |
| `resize <dir> [amount]` | forward the chord to Neovim, or `pane.resize` (default amount `0.03`) |
| `install [--root DIR]` | copy itself to `<plugin>/scripts/bin/herdr-splits` |
| `init [--force] [--print]` | write a `herdr-splits.conf` scaffold (instance policy), or print it |
| `doctor` | print resolved socket/conf/pane (and where the conf came from) and probe the server |

Exit codes match the Python scripts it replaced: `0` done (including "at the
edge, nothing to do"), `1` herdr unreachable / no current pane / RPC failure
— and, for `init`, an environment refusal (the file already exists, or it
cannot be written) — `2` usage error. Successful runs print nothing; herdr
captures the action's output into its plugin log.

**Compatibility with `scripts/herdr-nav.py` / `herdr-resize.py`.** The RPC
sequence, exit codes and edge/zoom decisions are identical (the fixtures in
`rust/tests` are transcribed from the Python unit tests). Three cosmetic
differences:

1. A failure *after* the first call prints one line
   (`herdr-splits nav: pane.edges failed: …`) and exits 1, instead of a Python
   traceback;
2. an unparsable `amount` prints `herdr-splits resize: invalid amount: '…'` and
   exits 2, instead of a traceback;
3. messages are prefixed `herdr-splits nav:` / `herdr-splits resize:` rather
   than `herdr-nav:` / `herdr-resize:`.

The vim-process-name matcher is ported bug-for-bug from the Python one, down to
its quirks (`gnvim` matches, `gnview` does not, `mvim` does not);
`tests/vimnames.tsv` pins it against a truth table generated from the Python
function.

**Binary name.** The manifest calls `scripts/bin/herdr-splits` with no
extension on every platform. That is a requirement, not a style choice: herdr
resolves a relative command against the plugin root and canonicalises it, and
Windows then runs that exact path without appending `PATHEXT` variants — it runs
an extensionless image, but not `<name>.exe` reached that way. On Windows the
installer therefore writes both: the extensionless image herdr launches, plus
`herdr-splits.exe` with the same bytes, because PowerShell and `cmd` refuse to
execute an extensionless file. Use whichever your shell accepts; both are
refreshed by the same `install` run.

### Why Rust instead of the Python scripts

Each keypress re-executed `python -S scripts/herdr-nav.py`, so the dominant cost
was interpreter start-up. Measured on Windows 11 (x86_64, Python 3.13, 50
interleaved runs per variant, same stubbed transport — no server, so the numbers
are the process cost the plugin owns):

| variant | min | P25 | median | P90 |
| --- | --- | --- | --- | --- |
| `python -S scripts/herdr-nav.py` (what the manifest used to run) | 98.2 | 103.3 | 109.4 | 137.0 |
| same script, slimmed (no `json`/`re`/`pathlib` imports) | 41.9 | 46.1 | 47.3 | 55.9 |
| **Rust helper** | **6.1** | **7.0** | **9.0** | **10.9** |

A bare `python -S` on an empty file costs ~55 ms on this machine, so the Rust
helper removes the interpreter start-up entirely; what is left is process
creation (~6 ms) plus a sub-millisecond RPC roundtrip. Reproduce with
`python bench/ab.py --scripts <dir> --bin scripts/bin/herdr-splits`.

### Float and embedded-float classification

Two predicates classify the current window before any nav/resize decision:

- `win.is_floating(winid)` — `nvim_win_get_config(winid).relative ~= ''` (Neovim's built-in float check).
- `win.is_embedded_floating_window(winid)` — true only for floats with `zindex < floating_zindex_max` (default 50); matches snacks/neo-tree/aerial float-mode sidebars.

True floats forward to Herdr. Embedded floats with a valid window-relative normal ancestor proxy that split's geometry; unresolved embedded floats no-op.

## Herdr Detection

Detected automatically through environment variables Herdr injects into every pane:

- `HERDR_ENV=1` — set when running inside a Herdr-managed pane
- `HERDR_PANE_ID` — the public pane ID (e.g., `w1:p1`)
- `HERDR_SOCKET_PATH` — the control socket the Neovim side talks to directly
  (one RPC roundtrip per action, well under a millisecond; no CLI subprocesses)
- `HERDR_BIN_PATH` — path to the herdr binary (used for conf-dir discovery and
  plugin auto-sync, not for navigation)

Detection answers one question: **should Herdr forward the key into this pane,
or move pane focus itself?** The answer has to identify the plugin, not merely
"a Vim process" — forwarding into a Vim without these mappings would swallow
the key.

- **Neovim** registers its pane with Herdr on `setup()`: a TTL'd token
  (`editor=nvim` via `pane.report_metadata`, refreshed by a heartbeat). The
  Herdr-side actions read it back over RPC.
- **Plain Vim** has no RPC transport for a token (Vimscript's `ch_open()` has
  no named-pipe support; Herdr uses a Windows named pipe). The Vim plugin
  appends `[herdr-splits]` to `titlestring` while `$HERDR_ENV=1`; Herdr reads
  it from `terminal_title_stripped`. This is the only reliable signal on
  Windows, where `pane.process_info` reports the pane shell instead of a
  program started from it. The marker is appended to a customized
  `titlestring` too; Vim restores the title on suspend/exit and reapplies it on
  resume.
- **Plugin-less vim/gvim on Unix** remains the last fallback via foreground
  process names. On Windows those names are not visible, so plain Vim needs the
  plugin's title marker.

Run `:checkhealth herdr-splits` (Neovim) to inspect transport and registration
state. In plain Vim, `:echo &titlestring` inside a Herdr pane should end with
`[herdr-splits]`.

## Comparison with vim-herdr-navigation

| Feature                     | vim-herdr-navigation | herdr-splits.nvim            |
| --------------------------- | -------------------- | ---------------------------- |
| Navigation                  | ✓                    | ✓                            |
| Plain Vim                   | ✓ (wincmd only)      | ✓ (MRU + instance policy)    |
| Resizing                    | ✗                    | ✓ (Neovim)                   |
| at_edge behaviours          | ✗                    | wrap / stop / split / custom |
| Count prefix                | ✗                    | ✓ (3<C-h> = move 3 left)     |
| Auto-unzoom + wrap-around   | ✗                    | ✓                            |
| Floating window handling    | ✗                    | ✓                            |
| Windows                     | ✗ (Linux/macOS only) | ✓                            |
| Herdr plugin (for keybinds) | ✓                    | ✓                            |
| Editor plugin               | ✓ (Neovim + Vim)     | ✓ (Neovim + Vim)             |

## Updating

The Neovim side (lua) auto-updates via lazy.nvim — it pulls `main` on
`LazySync`. Plain Vim updates with your Vim plugin manager (or by refreshing
its checkout). The Herdr side (the scripts under the Herdr-managed checkout)
does **not** auto-update by default: Herdr v1 has no `herdr plugin update`, so
a managed checkout stays frozen at the commit it was installed from.

**Option A — opt into automatic sync (recommended):** set
`auto_sync_herdr = true` in `setup()`. The plugin then reinstalls the
Herdr-managed checkout pinned to the exact commit lazy fetched, so the bash
scripts always match the lua side. It is a no-op when already in sync, when
in local-dev (`plugin link`) mode, or when the `herdr` binary is unavailable.

Also add a `build` hook to attempt synchronization after lazy installs or
updates when configuration is already active. Normal setup-time synchronization
still provides convergence on startup:

```lua
{
  'lmilojevicc/herdr-splits.nvim',
  build = ':lua require("herdr-splits").sync_herdr()',
  config = function()
    require('herdr-splits').setup({ auto_sync_herdr = true })
  end,
}
```

**Option B — manual:** after an update, re-run
`herdr plugin install lmilojevicc/herdr-splits.nvim` to refresh the
Herdr-side checkout. This also rebuilds the Rust helper, so an update that
changes `rust/` needs it (or a local `make build` in `plugin link` mode).

Release notes for each version live in [GitHub Releases](https://github.com/lmilojevicc/herdr-splits.nvim/releases).

## License

MIT

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
- **Smart unzoom + wrap-around**: Auto-unzooms when leaving a zoomed pane. At a layout edge, navigation wraps to the opposite side — crossing into a sibling Herdr pane when one exists (e.g. `pane | nvim` → `ctrl+l` wraps to the pane), otherwise wrapping within Neovim. Herdr-pane navigation also wraps around at edges (past the last pane → the first). Does not unzoom when moving between Neovim splits in one pane.
- **Count prefix support**: `3<C-h>` moves three splits left; `5<M-l>` resizes five steps right.

## Requirements

- Neovim ≥ 0.10 for the Lua plugin, **or** Vim ≥ 8.1 with `+job` for
  plain-Vim navigation
- [Herdr](https://herdr.dev) ≥ 0.7.0 (for plugin actions)
- Python 3 on `PATH` — the Herdr-side plugin actions run `python -S scripts/*.py`
  (direct socket RPC; the `.sh` scripts remain as a no-python fallback). Plain
  Vim also uses it for the edge handoff.

## Installation

Three steps — install on both the Herdr side and the Neovim side, then add keybinds.

### 1. Install the Herdr plugin

This provides the `nav-*` and `resize-*` actions that Herdr keybinds invoke.

```bash
# From GitHub (once published):
herdr plugin install lmilojevicc/herdr-splits.nvim

# Local development:
herdr plugin link /path/to/herdr-splits
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
      most_recently_focused = false,  -- tmux-style target choice at both levels: most recently focused adjacent window/pane wins (pane recency is shared with the Herdr-side scripts)
      herdr_bin = nil,                -- auto-detected from HERDR_BIN_PATH
      floating_zindex_max = 50,       -- floats with zindex < this are treated as embedded sidebars
      ignore_previewwindows = false,  -- opt-in: also treat previewwindow windows (e.g. .dbout) as sidebars
      -- auto_sync_herdr = true,      -- opt-in: sync Herdr-side scripts on update
      -- Managed keys — written to the generated herdr-splits.conf so the
      -- Herdr-side scripts agree. Pass Neovim notation (e.g. <M-Left>).
      nav_keys    = { left = '<C-h>', down = '<C-j>', up = '<C-k>', right = '<C-l>' },
      resize_keys = { left = '<M-h>', down = '<M-j>', up = '<M-k>', right = '<M-l>' },
      unzoom_on_nav = true,   -- auto-unzoom when navigating away from a zoomed pane
      nav_at_edge    = 'wrap', -- 'wrap' | 'stop' — Herdr pane-boundary wrap (distinct from at_edge)
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
- reads `most_recently_focused` from the shared `herdr-splits.conf` written by
  the Neovim `setup()` (override per editor with
  `let g:herdr_splits_most_recently_focused = 0` or `1`);
- hands the direction to `scripts/herdr-nav.py --pane-only` asynchronously when
  Vim is at its edge (Python is required for this step).

Configuration (set before the plugin loads):

```vim
let g:herdr_splits_python = 'python'          " interpreter for the edge handoff
let g:herdr_splits_script = ''                " optional explicit herdr-nav.py
let g:herdr_splits_most_recently_focused = 1  " override the shared conf
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

### Auto-unzoom and wrap-around

**Auto-unzoom.** When you navigate away from a **zoomed** pane, herdr-splits
unzooms first so the destination is visible — whether you're crossing into a
sibling Herdr pane or wrapping at a layout edge. It does not unzoom when
moving between Neovim splits inside the same pane.

**Wrap-around.** When you reach a layout edge in the direction you pressed
(nowhere further to go that way), the default `wrap` behaviour continues on
the opposite side:

- If a Herdr pane sits on the opposite side, focus crosses into it — so
  `pane | nvim` wraps `ctrl+l` back to the pane, and
  `pane | nvim win1 | win2` wraps from win2 to the pane. With `nav_at_edge=stop`
  (see below) this cross-to-opposite-pane is suppressed and the wrap stays
  within Neovim instead.
- Otherwise it wraps within Neovim (smart-splits default).
- Between plain Herdr panes, navigation wraps around at edges too (past the
  last pane → the first).
- Wrap works even when you're on a sidebar (dbui, neo-tree, quickfix, ...),
  so you can leave it at an edge. Parent-backed embedded floats use their
  containing split's geometry; unresolved embedded overlays remain gated.

**To disable auto-unzoom entirely**, pass `unzoom_on_nav = false` to `setup()`:

```lua
require('herdr-splits').setup({ unzoom_on_nav = false })
```

These options are written to a generated `herdr-splits.conf` (default
`~/.config/herdr/plugins/config/herdr-splits/herdr-splits.conf`; print the
path with `herdr plugin config-dir herdr-splits`) so the Herdr-side scripts
agree — you configure them once, from `setup()`. **Do not hand-edit that
file:** it is regenerated on every `setup()`. Any values already present are
adopted once the first time `setup()` runs after an upgrade, then
overwritten thereafter. Set `HERDR_SPLITS_CONFIG` to override the path on
both sides.

**To stop at layout edges instead of wrapping**, pass `nav_at_edge = 'stop'`:

```lua
require('herdr-splits').setup({ nav_at_edge = 'stop' })
```

This single switch controls wrap-across-boundary on **both** sides:

- **Plain Herdr panes:** `wrap` (the default) wraps to the opposite pane at an
  edge; `stop` halts.
- **Neovim edge wrap:** when Neovim's `at_edge='wrap'`, `wrap` (the default)
  lets the wrap cross to the Herdr pane on the opposite side (e.g. `ctrl+l` from
  the last nvim split lands on the pane); `stop` keeps the wrap within Neovim
  (e.g. `ctrl+l` from the last split cycles to the first split instead of
  leaving Neovim). `at_edge='stop'` halts regardless of `nav_at_edge`.

Note: `at_edge` (Neovim window-edge behaviour: `wrap`/`stop`/`split`/function)
and `nav_at_edge` (Herdr pane-boundary wrap: `wrap`/`stop`) are two different
things despite the shared name — `at_edge` is Neovim-side only and is not
written to the conf.

Note: when a pane is zoomed and `unzoom_on_nav=false`, the edge flags can't be
trusted so `stop` can't be detected on the plain-pane side — navigation proceeds
in the requested direction in that case. With the default `unzoom_on_nav=true`,
pressing toward an edge while zoomed first unzooms the pane (so the edge can be
detected) and then halts — the pane unzooms even though focus doesn't move.

**To remap the keys forwarded into Neovim**, pass `nav_keys` / `resize_keys`
to `setup()` in Neovim notation (e.g. `<M-Left>`). Override only the
directions you rebind; the rest keep their defaults. By default the scripts
forward `ctrl+h/j/k/l` for navigation and `alt+h/j/k/l` for resizing — the
chords the documented Neovim keymaps bind:

```lua
require('herdr-splits').setup({
  nav_keys    = { left = '<M-Left>', down = '<M-Down>', up = '<M-Up>', right = '<M-Right>' },
  resize_keys = { left = '<M-S-Left>', down = '<M-S-Down>', up = '<M-S-Up>', right = '<M-S-Right>' },
})
```

- Pass Neovim notation in `setup()` (`<C-h>`, `<M-Left>`); it is translated to the Herdr chord notation (`ctrl+h`, `alt+left`) the generated conf writes and `herdr pane send-keys` accepts.
- Any direction left unset keeps its default.
- The chord forwarded into Neovim must match the Neovim keymap that catches it: `<M-Left>` in `setup()` is the same key `alt+left` the script forwards, so keep your `vim.keymap.set` in sync with `nav_keys`.

> **Eager load required for custom keys.** The generated conf is written by
> `setup()`, so custom `nav_keys`/`resize_keys` need `setup()` to run eagerly
> or at `VeryLazy` — do **not** lazy-load this plugin _only_ from the same
> custom key mappings it must publish. Otherwise the conf carrying your custom
> chords isn't written before Herdr forwards them, and those keys can't reach
> Neovim to trigger the lazy load. The recommended spec above already uses
> `event = 'VeryLazy'`.

## Local development

Both sides load straight from your clone, so edits take effect immediately
(on the Herdr side; Neovim picks them up on reload/restart).

1. **Link the Herdr plugin** to your local repo:

   ```bash
   herdr plugin link /path/to/herdr-splits
   ```

2. **Point lazy.nvim at the same path** — change the first line of the spec
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

## Lua API

```lua
require('herdr-splits').setup(opts)

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
  ├─ Is the focused pane running an editor (Neovim / plain Vim with this plugin)?
  │   │
  │   ├─ YES → forward "ctrl+h" into that pane
  │   │         └─ the editor plugin receives it
  │   │              ├─ Has a window to the left? → wincmd h
  │   │              └─ At the editor edge? → cross into the Herdr pane
  │   │
  │   └─ NO (plain shell, etc.) → herdr pane focus --direction left
  │
  └─ Result: same keys move you everywhere
```

Neovim performs the edge crossing in-process over the control socket. Plain Vim
cannot open Herdr's socket from Vimscript, so at its edge it spawns
`scripts/herdr-nav.py --pane-only` (direct RPC) asynchronously; the pane-level
decision (MRU, unzoom, wrap) stays in one implementation.

### Navigation

```text
1. Try moving within the editor (wincmd h/j/k/l)
   ├─ Window changed → done (stayed within the editor's splits)
   └─ Window didn't change → at the editor edge
        ├─ Zoomed? → unzoom first (neighbours are reliable once visible)
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

### Resizing

```text
1. Only one Neovim window in this dimension AND fills terminal?
   └─ Yes → herdr pane resize --direction --amount <ratio> → done
2. Otherwise:
   └─ Native Neovim resize with position-aware +/- operators
```

Plain Vim is treated like a plain pane for resizing: the `resize-*` actions
resize the Herdr pane directly, because the Vim adapter implements navigation
but no resize mappings.

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
| Plain Vim                   | ✓ (wincmd only)      | ✓ (MRU + shared conf)        |
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
Herdr-side scripts.

Release notes for each version live in [GitHub Releases](https://github.com/lmilojevicc/herdr-splits.nvim/releases).

## License

MIT

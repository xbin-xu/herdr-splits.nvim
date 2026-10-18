# Contributing to herdr-splits.nvim

Thanks for your interest in improving herdr-splits.nvim! This guide covers how
to set up a local development environment and submit changes.

herdr-splits.nvim is a Neovim plugin (Lua) that also ships a Herdr-side plugin
(the bash scripts under `scripts/` plus the `herdr-plugin.toml` manifest). The
two halves cooperate — please consider both when making changes to navigation
or resize behaviour.

## Requirements

- Neovim ≥ 0.10
- A Rust toolchain (`cargo`) — the Herdr-side helper is built from `rust/`
- [Herdr](https://herdr.dev) ≥ 0.8.0 (only needed to exercise cross-pane
  behaviour; you can develop the Lua side without a running Herdr session)
- A Herdr-compatible terminal with Option/Alt enabled (see the README's macOS
  notes if you're on macOS)

## Repository layout

```
plugin/herdr-splits.lua        Neovim entrypoint
lua/herdr-splits/              Neovim plugin source
  init.lua                     setup() + public API
  nav.lua resize.lua           navigation and resizing
  win.lua                      float / sidebar classification
  herdr.lua                    Herdr operations over RPC
  rpc.lua                      control-socket RPC client (libuv pipe)
  geometry.lua                 directional candidate search (shared rule)
  register.lua                 pane registration token + heartbeat
  sync.lua                     Herdr-side checkout syncing
  conf.lua paths.lua           shared conf file + plugin paths
  config.lua                   resolved config
  health.lua                   :checkhealth herdr-splits
rust/                          Herdr-side helper (what the actions run)
  src/main.rs                  CLI dispatch
  src/cli.rs                   argument parsing, usage, exit codes
  src/ctx.rs                   socket/conf/pane-id resolution
  src/rpc.rs                   NDJSON client (unix socket / named pipe) + replay/trace
  src/panes.rs                 typed pane.* operations
  src/editor.rs vimname.rs     editor detection
  src/geometry.rs              directional candidate search (same rule as Lua)
  src/mru.rs                   pane recency (shared mru-panes file)
  src/nav.rs resize.rs         decision trees
  src/config.rs                herdr-splits.conf parsing
  src/install.rs doctor.rs     self-install and diagnostics
  tests/e2e.rs                 end-to-end tests over the real binary
  tests/vimnames.tsv           truth table generated from the Python matcher
bench/ab.py                    Python-vs-Rust timing harness
scripts/bin/herdr-splits       installed helper (gitignored; built)
scripts/build.sh build.ps1     build + install the helper
scripts/herdr-nav.py           reference implementation (no longer invoked)
scripts/herdr-nav.sh           bash fallback (no toolchain at all)
scripts/herdr-resize.py        reference implementation (no longer invoked)
scripts/herdr-resize.sh        bash fallback (no toolchain at all)
scripts/minimal_init.lua       Isolated Neovim test bootstrap
tests/                         mini.test suites + Python unit tests
Makefile                       Local and CI validation commands
herdr-plugin.toml              Herdr plugin manifest (actions + metadata)
```

## Local development setup

### Neovim side

Point your plugin manager at a local clone instead of the published repo:

```lua
-- lazy.nvim
{
  dir = '~/Projects/herdr-splits',
  cond = vim.env.HERDR_ENV == '1',
  config = function()
    require('herdr-splits').setup({})
  end,
}
```

Changes to files under `lua/` take effect after `:source` or a Neovim restart
(unless your plugin manager hot-reloads).

### Herdr side

Link the clone so Herdr loads its scripts from your working tree:

```bash
herdr plugin link /path/to/herdr-splits
```

`plugin link` does not run the manifest's `[[build]]` commands, so build the
helper once (and again after every change under `rust/`):

```bash
./scripts/build.sh          # Windows: pwsh -File scripts\build.ps1
```

After changing `herdr-plugin.toml` or the helper itself, run
`herdr server reload-config` to pick the manifest changes up.

## Coding style

- Match the surrounding Lua style — indentation, quoting, and table layout in
  the file you're editing are the source of truth.
- Run [StyLua](https://github.com/JohnnyMorganz/StyLua) if you use it, but note
  that no `.stylua.toml` is committed yet; when one is added it will be the
  canonical formatter config.
- Run [luacheck](https://github.com/mpeterv/luacheck) where practical to catch
  undefined globals and unused variables.
- Rust: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` must be
  clean; CI enforces both. Keep the crate dependency-free — installs must work
  offline on a machine that only has the toolchain.
- Keep the bash scripts POSIX-friendly where possible; they run on both Linux
  and macOS as declared in `herdr-plugin.toml`.

## Validation

The Make targets initialize and update the pinned test dependency automatically.
To prepare it separately, run `make deps`.

Run the automated checks with:

```bash
make test         # isolated mini.test suite in headless Neovim
make test-vim     # plain-Vim title tests
make test-py      # Python unit tests (the reference implementation)
make test-rust    # Rust unit + end-to-end tests (no Herdr server needed)
make check-shell  # bash syntax checks for the shipped scripts
make build        # cargo build --release + install into scripts/bin/
make check        # all of the above
```

The Rust end-to-end tests drive the real binary with `--replay`, which answers
RPC calls from a fixture instead of the socket. That means they pass on a
machine where Herdr is not running (or runs elevated and refuses ordinary
clients), and they pin the exact RPC sequence, stdout/stderr text and exit code.

Cross-pane behaviour still requires manual validation in a Herdr session:

1. Load the plugin in Neovim inside a Herdr session (`HERDR_ENV=1`).
2. Run `:checkhealth herdr-splits` and confirm it reports no failures (it also
   reports whether `scripts/bin/herdr-splits` is built).
3. Run `scripts/bin/herdr-splits doctor` and confirm the resolved socket, conf
   path, current pane and detected editor are what you expect.
4. Exercise navigation (`<C-h/j/k/l>`) and resizing (`<M-h/j/k/l>`) across both
   Neovim splits and Herdr panes, including the edge cases you changed:
   - `at_edge` behaviour (`wrap` / `stop` / `split` / custom)
   - count prefixes (e.g. `3<C-h>`)
   - floating windows and embedded-sidebar floats
   - auto-unzoom when crossing into a sibling Herdr pane
   - plain Vim at its edge (the `--pane-only` hand-off)
5. `herdr plugin log list --plugin herdr-splits` should show
   `scripts/bin/herdr-splits …` invocations, not `python -S …`.

## Submitting changes

1. Fork the repo and create a branch from `main`.
2. Make focused commits with descriptive messages.
3. Open a pull request against `main` and fill in the PR template — include the
   manual validation steps you ran.
4. If your change affects user-facing behaviour or config, update `README.md`
   and, for manifest-level changes, `herdr-plugin.toml`.

Keep PRs scoped to one concern. Large changes are easier to review when split
into smaller, independently understandable commits.

## Reporting bugs and ideas

Use the GitHub issue templates (bug report / feature request). Include the
output of `nvim --version`, your plugin manager, the Herdr version
(`herdr --version`), and a minimal reproduction.

## License

By contributing you agree that your contributions are licensed under the MIT
License, the same terms as the rest of the project (see `LICENSE`).

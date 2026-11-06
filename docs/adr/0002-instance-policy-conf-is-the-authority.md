# The instance policy file is the authority, not `setup()`

`unzoom_on_nav`, `nav_at_edge`, `most_recently_focused` and the four forwarded chords are decided by one file, `herdr-splits.conf`, which the Herdr-side helper, plain Vim and Neovim all read. The helper creates it (with the shipped defaults) when it is missing, and no editor writes it unless explicitly asked with `setup({ overwrite_conf = true })`. This replaces the model introduced in f95cbdb, where a Neovim `setup()` was the sole writer and every other participant read its projection.

The reason is that the settings are *not* Neovim's: they are half-executed by Neovim (it unzooms the pane it is leaving, it picks the MRU pane itself over RPC) and half by the helper (plain panes, and every chord arriving outside an editor). With `setup()` as the writer, the policy of a whole Herdr instance — including plain shell panes, and including plain Vim, which has no writer at all — was decided by whichever Neovim started last. That is a real failure mode (a `:Lazy reload` silently changing how shells navigate) and it made hand-editing the file futile, so the only way to express a policy was Lua.

## Considered options

- **Keep `setup()` as the sole writer (status quo).** Simplest for a Neovim-only user, and it keeps `setup()` as the one place to configure the plugin. Rejected because it makes an editor the authority over a multiplexer-level policy, cannot express "both editors agree, neither is special", and silently reverts hand edits.
- **Let both editors write (last writer wins).** Rejected outright: Neovim caches the resolved values at `setup()` time (`config.lua`), so a later write from Vim would never reach Neovim's in-process decisions — the two layers would disagree with no way to notice. Last-write-wins is not consistency when one reader froze its copy.
- **Hand-written file, nobody writes at all.** This is the chosen model; `init` exists so "hand-written" starts from a valid scaffold of the effective defaults instead of a blank file.
- **Have the editor fall back to the file only when an opt is unset, and publish otherwise.** Kept as an opt-in (`overwrite_conf`), not the default: publishing re-creates the "one editor decides for everyone" problem for anyone who sets an opt.

## Consequences

- A missing conf is a normal state: every reader falls back to the same shipped defaults (`Config::defaults`, `managed_defaults`, the Python fallback), so nothing requires ordering between creating the file and starting an editor. Because the scaffold writes exactly those defaults, creating the file cannot change behaviour — which also makes "delete the file" a working reset.
- The helper (`nav`, `resize`, `install`) creates the file when it is missing. That keeps the writer inside the program that owns the semantics, and means a fresh install has something to edit without anyone having to know `init` exists. `init` remains for the explicit cases: write now, `--print`, `--force` to reset.
- Neovim reads the conf once, in `setup()`, and therefore needs `:HerdrSplitsReload` (or a restart) to see an edit; the helper and plain Vim read it per keypress/chord. Chords additionally need their terminal buffers reopened, because the maps are installed per buffer.
- `overwrite_conf = true` still exists and still takes over an unmarked file, stamping the marker. That mode is the deliberate exception, and it announces the adoption when it happens.
- `nav_keys` / `resize_keys` in `setup()` without `overwrite_conf` are ignored with a warning: Herdr forwards the chord, so a Lua-only chord override would desync the editor's terminal maps from what Herdr actually sends.

## Revision: created by the helper, defaults flipped

The first cut made creation a purely manual act (`herdr-splits init`), which left a fresh install with no file at all — correct, but it made the policy invisible until someone knew about the command. The helper now writes the scaffold itself when the file is missing (`nav`, `resize`, `install`), which keeps the writer in the program that owns the semantics while removing the discovery step. The scaffold is generated from `Config::defaults`, so it cannot drift from what a missing conf means, and creating it never changes behaviour.

The shipped defaults also moved to this setup's taste: `most_recently_focused=true` (was `false`) and `unzoom_on_nav=false` (was `true`, so a zoomed pane now stays zoomed and the zoom follows the focus); `nav_at_edge` stays `wrap`. All four readers carry the same numbers — Rust `Config::defaults`, the Neovim `managed_defaults`, the Python fallback, and the scaffold — because a single divergent copy would re-create exactly the split-brain this ADR removes.

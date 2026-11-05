" herdr-splits — plain-Vim runtime for herdr-splits.
"
" The navigation logic lives in autoload/herdr_splits.vim. Neovim is
" handled by the Lua plugin (plugin/herdr-splits.lua) and must not load
" this file. Mappings are defined here, only inside a Herdr pane; the
" vimrc still owns the vim-tmux-navigator gate (g:tmux_navigator_no_mappings)
" because that must be set before any plugin script runs.

if has('nvim')
  finish
endif
if exists('g:loaded_herdr_splits')
  finish
endif
let g:loaded_herdr_splits = 1

" Navigation helper for the edge hand-off. Empty means "use this checkout's
" own build" (scripts/bin/herdr-splits, produced by the manifest's build
" commands); set it to an absolute path to use a different build.
let g:herdr_splits_bin = get(g:, 'herdr_splits_bin', '')
" Fallback only: interpreter and script used when the Rust helper has not been
" built in this checkout. Override in your vimrc before the plugin loads if
" python is not on PATH.
let g:herdr_splits_python = get(g:, 'herdr_splits_python', 'python')
" Optional explicit path to herdr-nav.py; empty uses the copy shipped in this
" plugin's own checkout (scripts/herdr-nav.py).
let g:herdr_splits_script = get(g:, 'herdr_splits_script', '')
" Terminal-mode navigation switch (default on). Neovim scopes its equivalent
" (`terminal_nav`) with an allow-list because there it only *replaces*
" interception a distribution already does; here the tnoremap below is the only
" way to leave a terminal split, so it stays on and is install-time opt-out.
" To keep the chords for one terminal buffer (or a terminal plugin), win the
" mapping yourself — buffer-local beats global:
"   autocmd TerminalOpen * tnoremap <buffer> <C-h> <C-h>
let g:herdr_splits_terminal_nav = get(g:, 'herdr_splits_terminal_nav', 1)

" Advertise this plugin in the terminal title. nvim registers a TTL'd pane
" token over RPC; plain Vim has no RPC transport for one, and on Windows
" pane.process_info only sees the pane shell (the MSYS spawn layer hides
" programs started from it). The title is the only always-current channel
" herdr can read. Append a marker to the user's title (the default title
" format when unset) so detection means "the Vim plugin is loaded", not
" merely "some Vim".
"
" Vim itself restores the title on suspend/exit and reapplies it on resume
" (mch_restore_title / maketitle / resettitle); `set notitle` would only
" stop updates, not clear the current title, so no autocmds are needed.
if $HERDR_ENV ==# '1'
  let &titlestring = (&titlestring ==# '' ? '%t - VIM' : &titlestring)
        \ . ' [herdr-splits]'
  set title
endif

" Mappings — only inside a Herdr pane. Outside one, leave whatever owns
" C-hjkl alone (vim-tmux-navigator keeps working for tmux sessions).
if $HERDR_ENV ==# '1'
  nnoremap <silent> <C-h> :<C-U>call herdr_splits#nav('h')<CR>
  nnoremap <silent> <C-j> :<C-U>call herdr_splits#nav('j')<CR>
  nnoremap <silent> <C-k> :<C-U>call herdr_splits#nav('k')<CR>
  nnoremap <silent> <C-l> :<C-U>call herdr_splits#nav('l')<CR>
  if g:herdr_splits_terminal_nav
    tnoremap <silent> <C-h> <C-\><C-n>:<C-U>call herdr_splits#nav('h', 1)<CR>
    tnoremap <silent> <C-j> <C-\><C-n>:<C-U>call herdr_splits#nav('j', 1)<CR>
    tnoremap <silent> <C-k> <C-\><C-n>:<C-U>call herdr_splits#nav('k', 1)<CR>
    tnoremap <silent> <C-l> <C-\><C-n>:<C-U>call herdr_splits#nav('l', 1)<CR>
  endif
endif

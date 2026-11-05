" Argument wiring for the edge hand-off — no pty, no server.
"
" Run headless:
"   vim -u NONE -N --not-a-term -S tests/vim/test_handoff_args.vim </dev/null
"
" The shared conf is one file per Herdr instance and only a Neovim setup()
" writes it, so a plain-Vim preference travels on the command line instead:
" g:herdr_splits_nav_at_edge / g:herdr_splits_unzoom_on_nav /
" g:herdr_splits_most_recently_focused become --nav-at-edge / --unzoom /
" --most-recently-focused. Unset must mean "send nothing" so the conf decides.
"
" s:nav_command() is script-local; Vim 9's getscriptinfo() gives the script id
" so the test can call it without exporting anything just for tests.

let s:root = expand('<sfile>:p:h:h:h')

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_handoff_args: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

execute 'set runtimepath^=' . fnameescape(s:root)
let $HERDR_ENV = '1'
execute 'source ' . fnameescape(s:root . '/plugin/herdr-splits.vim')

" Autoload scripts load lazily: source it so getscriptinfo() can see it (and so
" the test never depends on a function call having happened first).
runtime autoload/herdr_splits.vim

let s:sid = 0
for s:info in getscriptinfo()
  if s:info.name =~# 'autoload[/\\]herdr_splits\.vim$'
    let s:sid = s:info.sid
  endif
endfor
call s:check(s:sid > 0, 1, 'autoload script id resolved')

function! s:nav_command(direction) abort
  return call(function('<SNR>' . s:sid . '_nav_command'), [a:direction])
endfunction

" A pinned binary keeps the branch deterministic (the checkout may well have a
" built scripts/bin/herdr-splits).
let g:herdr_splits_bin = '/example/herdr-splits'

" --- no overrides: the conf decides ---------------------------------------
unlet! g:herdr_splits_nav_at_edge g:herdr_splits_unzoom_on_nav
unlet! g:herdr_splits_most_recently_focused
call s:check(
      \ s:nav_command('left'),
      \ ['/example/herdr-splits', 'nav', '--pane-only', 'left'],
      \ 'no overrides')

" --- all three overrides ---------------------------------------------------
let g:herdr_splits_nav_at_edge = 'stop'
let g:herdr_splits_unzoom_on_nav = 0
let g:herdr_splits_most_recently_focused = 1
call s:check(
      \ s:nav_command('down'),
      \ ['/example/herdr-splits', 'nav', '--pane-only',
      \  '--nav-at-edge=stop', '--unzoom=off', '--most-recently-focused=on', 'down'],
      \ 'all overrides')

" --- the opposite values ------------------------------------------------
let g:herdr_splits_nav_at_edge = 'wrap'
let g:herdr_splits_unzoom_on_nav = 1
let g:herdr_splits_most_recently_focused = 0
call s:check(
      \ s:nav_command('up'),
      \ ['/example/herdr-splits', 'nav', '--pane-only',
      \  '--nav-at-edge=wrap', '--unzoom=on', '--most-recently-focused=off', 'up'],
      \ 'overrides flipped')

" --- an invalid value is dropped, not passed on (one-shot warning) --------
let g:herdr_splits_nav_at_edge = 'sometimes'
call s:check(
      \ s:nav_command('right'),
      \ ['/example/herdr-splits', 'nav', '--pane-only',
      \  '--unzoom=on', '--most-recently-focused=off', 'right'],
      \ 'invalid nav_at_edge ignored')

" --- the Python fallback gets the same arguments --------------------------
" Only reachable when this checkout has no built helper; with one, the Rust
" binary wins the preference order (that is what the branch above asserts).
if !executable(s:root . '/scripts/bin/herdr-splits')
  unlet! g:herdr_splits_bin
  let g:herdr_splits_nav_at_edge = 'stop'
  let g:herdr_splits_script = s:root . '/scripts/herdr-nav.py'
  call s:check(
        \ s:nav_command('left'),
        \ ['python', '-S', s:root . '/scripts/herdr-nav.py', '--pane-only',
        \  '--nav-at-edge=stop', '--unzoom=on', '--most-recently-focused=off', 'left'],
        \ 'python fallback arguments')
endif

qa!

" Terminal-Job mode must survive a nav chord — regression test.
"
" OPT-IN: this is the only Vim test that opens real ptys (term_start). Each one
" is a ConPTY console on Windows, and a live pty inherits stdout, so a hung run
" looks like "make test-vim spawns console.exe forever and eats CPU". It is
" therefore skipped unless HERDR_SPLITS_TEST_TERM=1 (CI sets it), and it wipes
" the terminal buffers before quitting so no job outlives Vim.
"
" Run it explicitly:
"   HERDR_SPLITS_TEST_TERM=1 vim -u NONE -N --not-a-term -S tests/vim/test_nav_term_mode.vim </dev/null
"
if $HERDR_SPLITS_TEST_TERM !=# '1'
  " Skipped: opt-in pty test (see above).
  qa!
endif

" The t-mode mapping starts with <C-\><C-n> (a Terminal-Job mapping cannot run
" an Ex command otherwise), so it always leaves Terminal-Job mode first. Vim
" then draws the shell scrollback like a normal buffer — line numbers, cursor in
" the output — which is why herdr_splits#nav() takes a `from_term` argument and
" switches the window back to Terminal-Job mode when the cursor ends up in a
" terminal window. Without it, pressing <C-j> at the bottom edge left the window
" in Terminal-Normal mode for good.

let s:root = expand('<sfile>:p:h:h:h')

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_nav_term_mode: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

execute 'set runtimepath^=' . fnameescape(s:root)
let $HERDR_ENV = ''  " no Herdr hand-off here: only the Vim-side window move
execute 'source ' . fnameescape(s:root . '/plugin/herdr-splits.vim')

function! s:term() abort
  let b = term_start(&shell, {'term_rows': 5, 'curwin': 1})
  call term_wait(b, 200)
  return b
endfunction

" Layout: a text window on top, a terminal in the bottom window (so there is no
" Vim window below it — the case that used to strand the cursor).
split
wincmd j
let s:bottom = s:term()
call s:check(mode(), 't', 'terminal starts in Terminal-Job mode')

" A) chord with nowhere to go: back to Terminal-Job, same window.
call feedkeys("\<C-\>\<C-n>", 'nx')
call herdr_splits#nav('j', 1)
call s:check(mode(), 't', 'edge: stays in Terminal-Job mode')
call s:check(&buftype, 'terminal', 'edge: still the terminal buffer')

" B) a deliberate Terminal-Normal chord must not be overridden.
call feedkeys("\<C-\>\<C-n>", 'nx')
call herdr_splits#nav('j')
call s:check(mode(), 'n', 'Terminal-Normal chord stays in normal mode')

" C) chord into a text window: normal mode, as expected.
call feedkeys("\<C-\>\<C-n>", 'nx')
call herdr_splits#nav('k', 1)
call s:check(mode(), 'n', 'text window: normal mode')
call s:check(&buftype, '', 'text window: not a terminal')

" D) chord into another terminal window: Terminal-Job there too, so you can
"    keep typing; a bare wincmd leaves it in Terminal-Normal mode instead.
wincmd k
let s:top = s:term()
wincmd j
call feedkeys("\<C-\>\<C-n>", 'nx')
call herdr_splits#nav('k', 1)
call s:check(mode(), 't', 'into a terminal window: Terminal-Job mode')
call s:check(&buftype, 'terminal', 'into a terminal window: terminal buffer')

" Tear the ptys down explicitly. `job_stop(..., 'kill')` is the important part:
" on Windows every terminal is a ConPTY console, and a conhost whose parent
" (Vim) exits without killing the job survives as an orphan that spins a core —
" nobody reads its output pipe any more. Verified the hard way.
for s:b in term_list()
  silent! call job_stop(term_getjob(s:b), 'kill')
endfor
silent! %bwipeout!

qa!

" Terminal-Job mode must survive a nav chord — regression test.
"
" OPT-IN: this is the only Vim test that opens real ptys (term_start). Each one
" is a ConPTY console on Windows, and a live pty inherits stdout, so a hung run
" looks like "make test-vim spawns console.exe forever and eats CPU". It is
" therefore skipped unless HERDR_SPLITS_TEST_TERM=1 (CI sets it), and it tears
" the ptys down in a catch-all (see s:teardown) so no job or console host
" outlives Vim — including when an assertion fails.
"
" Run it explicitly:
"   HERDR_SPLITS_TEST_TERM=1 vim -u NONE -N --not-a-term -S tests/vim/test_nav_term_mode.vim </dev/null
"
if $HERDR_SPLITS_TEST_TERM !=# '1'
  " Skipped: opt-in pty test (see above).
  qa!
endif

" The terminal-mode chord is `<prefix>:call herdr_splits#nav(dir)<CR>`, where
" {prefix} is 'termwinkey' (CTRL-W by default). `CTRL-W :` runs one Ex command
" from Terminal-Job mode and returns to it (:h t_CTRL-W_:), the way <C-o> borrows
" Normal mode from Insert mode. The previous version sent <C-\><C-n> instead:
" that always leaves Terminal-Job mode, so Vim then drew the shell scrollback
" like a normal buffer — line numbers, cursor in the output — for as long as the
" cursor stayed elsewhere. These assertions pin the difference: a chord never
" changes the mode of the terminal window it is pressed in, and <C-\><C-n>
" remains the user's own gesture into the scrollback.

let s:root = expand('<sfile>:p:h:h:h')
set nomore

function! s:fail(msg) abort
  throw 'test_nav_term_mode: ' . a:msg
endfunction

function! s:check(got, want, label) abort
  if a:got !=# a:want
    call s:fail(a:label . ': got ' . string(a:got) . ', want ' . string(a:want))
  endif
endfunction

execute 'set runtimepath^=' . fnameescape(s:root)
let $HERDR_ENV = ''  " no Herdr hand-off here: only the Vim-side window move
execute 'source' fnameescape(s:root . '/plugin/herdr-splits.vim')

function! s:term() abort
  let b = term_start(&shell, {'term_rows': 5, 'curwin': 1})
  call term_wait(b, 200)
  return b
endfunction

" mode() is global; win_execute() runs with {winid} current, so it reports the
" mode that window's buffer is in.
function! s:mode_in(winid) abort
  call win_execute(a:winid, 'let g:test_mode_there = mode()')
  return get(g:, 'test_mode_there', '?')
endfunction

" kill the jobs first, then wait for them to die: `bwipeout!` on a window whose
" job is still running puts up a prompt, and a prompt on a `</dev/null` stdin
" hangs Vim (and leaves the ConPTY console host spinning after it is killed).
function! s:teardown() abort
  for b in term_list()
    silent! call job_stop(term_getjob(b), 'kill')
  endfor
  sleep 300m
  silent! tabonly!
  silent! only!
  silent! %bwipeout!
endfunction

function! s:run() abort
  " Layout: a text window on top, a terminal in the bottom window.
  split
  wincmd j
  call s:term()
  call s:check(mode(), 't', 'terminal starts in Terminal-Job mode')
  let s:bottom_win = win_getid()

  " A) The chord itself: a borrowed CTRL-W command line, never a mode switch.
  let s:chord = herdr_splits#term_nav('k')
  call s:check(strpart(s:chord, 0, 1), "\<C-W>", 'chord starts with CTRL-W')
  call s:check(stridx(s:chord, "\<C-\>\<C-n>"), -1, 'chord sends no <C-\><C-n>')

  " B) Chord with nowhere to go: still Terminal-Job, same window.
  call feedkeys(herdr_splits#term_nav('j'), 'nx')
  call s:check(mode(), 't', 'edge: stays in Terminal-Job mode')
  call s:check(&buftype, 'terminal', 'edge: still the terminal buffer')

  " C) Chord into a text window: the cursor moves, and the terminal window it
  "    came from KEEPS Terminal-Job mode (the regression: it used to be left in
  "    Terminal-Normal mode, so the shell scrollback was drawn with line numbers).
  call feedkeys(herdr_splits#term_nav('k'), 'nx')
  call s:check(mode(), 'n', 'text window: normal mode')
  call s:check(&buftype, '', 'text window: not a terminal')
  call s:check(s:mode_in(s:bottom_win), 't',
        \ 'source terminal window stayed in Terminal-Job mode')

  " D) Chord back down: still Terminal-Job, because it never left.
  call feedkeys(herdr_splits#term_nav('j'), 'nx')
  call s:check(mode(), 't', 'back in the terminal: Terminal-Job mode')
  call s:check(&buftype, 'terminal', 'back in the terminal: terminal buffer')

  " E) <C-\><C-n> is still the user's own way into the scrollback.
  call feedkeys("\<C-\>\<C-n>", 'nx')
  call s:check(mode(), 'n', 'deliberate <C-\><C-n> enters Terminal-Normal mode')

  " F) A chord from Terminal-Normal mode navigates and leaves that mode alone:
  "    the plugin does not force Job mode on a window the user parked.
  call feedkeys(herdr_splits#term_nav('k'), 'nx')
  call s:check(mode(), 'n', 'text window: normal mode')
  call s:check(s:mode_in(s:bottom_win), 'n',
        \ 'a deliberately parked terminal stays in Terminal-Normal mode')

  " G) Arriving in a terminal window from elsewhere is left to Vim as well; `i`
  "    (or the user's own autocmd) is what returns it to Terminal-Job mode.
  call feedkeys(herdr_splits#term_nav('j'), 'nx')
  call s:check(mode(), 'n', 'arriving in Terminal-Normal: mode untouched')
  call feedkeys('i', 'nx')
  call s:check(mode(), 't', 'i returns the terminal to Terminal-Job mode')
endfunction

try
  call s:run()
catch
  call s:teardown()
  echomsg v:exception
  cquit 1
endtry
call s:teardown()

qa!

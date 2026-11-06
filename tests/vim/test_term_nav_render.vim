" Does a nav chord leave line numbers over the shell? — rendered end-to-end test.
"
" OPT-IN: like test_nav_term_mode.vim this opens real ptys (a nested Vim inside
" a ConPTY). Skipped unless HERDR_SPLITS_TEST_TERM=1, and it tears the jobs down
" in a catch-all.
"
" Run it explicitly:
"   HERDR_SPLITS_TEST_TERM=1 vim -u NONE -N --not-a-term -S tests/vim/test_term_nav_render.vim </dev/null
"
" Why a nested Vim instead of asserting mode(): the bug is visual. A chord that
" leaves Terminal-Job mode is not an error — Vim is happy to park the window in
" Terminal-Normal mode and draw the shell scrollback like a buffer, gutters
" included. Only the drawn screen can tell the two apart, and only a real pty
" resolves a t-mode mapping at all (feedkeys()/`:normal` cannot). So:
"
"   outer Vim  --term_start-->  inner Vim (real pty, real keypresses)
"              --term_sendkeys--> keys, --getbufline()--> the drawn screen
"
" The inner session is: numbered text buffer on top, `:terminal` running a shell
" that printed MARKER on the bottom. Numbering is on, so the text window rows
" prove the detector works ('1 TOP-A'); the terminal rows must stay gutter-free.

if $HERDR_SPLITS_TEST_TERM !=# '1'
  " Skipped: opt-in pty test (see above).
  qa!
endif

let s:root = expand('<sfile>:p:h:h:h')
set nomore

" The inner session prints its marker with a POSIX shell one-liner
" (`echo ...; sleep`), so it needs a sh-like 'shell'. Nothing here depends on
" Windows.
if &shell !~# 'sh'
  echomsg 'test_term_nav_render: skipped (&shell is not sh-like: ' . &shell . ')'
  qa!
endif

function! s:fail(msg) abort
  throw 'test_term_nav_render: ' . a:msg
endfunction

function! s:check(got, want, label) abort
  if a:got !=# a:want
    call s:fail(a:label . ': got ' . string(a:got) . ', want ' . string(a:want))
  endif
endfunction

" Row containing {needle}, or '' when it is not on screen.
function! s:row(lines, needle) abort
  for l in a:lines
    if stridx(l, a:needle) >= 0
      return l
    endif
  endfor
  return ''
endfunction

" A line-number gutter: leading blanks then digits then a space.
function! s:numbered(row) abort
  return a:row =~# '^\s*\d\+\s'
endfunction

" --- inner session --------------------------------------------------------
" A separate fixture file keeps the nested quoting out of this test: the inner
" Vim finds the checkout through $HERDR_SPLITS_TEST_ROOT.
let $HERDR_SPLITS_TEST_ROOT = s:root
let s:inner = s:root . '/tests/vim/fixtures/term_nav_inner.vim'  " repo file, never deleted

let s:rows = 24
let s:pane = term_start(['vim', '-u', 'NONE', '-N', '-i', 'NONE', '-S', s:inner],
      \ {'term_rows': s:rows, 'term_cols': 90, 'curwin': 1})
call term_wait(s:pane, 2500)

" The drawn screen: term_getline() reads the terminal's *grid*, one row at a
" time — getbufline() would only give the scrollback (nothing, while the shell
" is still on screen).
function! s:screen() abort
  call term_wait(s:pane, 400)
  return map(range(1, s:rows), 'term_getline(s:pane, v:val)')
endfunction

" Ask the inner Vim to leave on its own steam: kill *its* terminal job, then
" quit. Force-killing the ConPTY out from under a live Vim is what makes the
" msys/Cygwin runtime print "fhandler_console::pcon_hand_over_proc: Acquiring
" pcon_ho_mutex failed." at exit: that process was started inside a pseudo
" console, and the pty's switch mutex (PIPE_SW_MUTEX) is already gone when it
" tries to hand the console back. Leaving in order avoids the message — and the
" orphan console hosts that come with a killed pty.
function! s:teardown() abort
  if exists('s:pane') && term_getstatus(s:pane) =~# 'running'
    silent! call term_sendkeys(s:pane,
          \ "\<C-\>\<C-n>:call job_stop(term_getjob(winbufnr(winnr('$'))), 'kill') | qall!\<CR>")
    call term_wait(s:pane, 800)
    if term_getstatus(s:pane) =~# 'running'
      silent! call job_stop(term_getjob(s:pane), 'kill')
    endif
  endif
  sleep 500m
  silent! %bwipeout!
endfunction

function! s:run() abort
  " The inner Vim must be in Terminal-Job mode, with the shell's output on screen.
  let l:lines = s:screen()
  let l:top = s:row(l:lines, 'TOP-A')
  call s:check(l:top ==# '' ? '' : s:numbered(l:top), 1,
        \ 'control: the text window is numbered (detector works)')
  let l:marker = s:row(l:lines, 'MARKER-LINE')
  call s:check(l:marker ==# '' ? '' : s:numbered(l:marker), 0,
        \ 'control: the terminal window starts gutter-free')

  " Positive control — a deliberate <C-\><C-n> does park the window in
  " Terminal-Normal mode, and then Vim DOES draw the gutter. This is exactly the
  " regression, spelled out.
  call term_sendkeys(s:pane, "\<C-\>\<C-n>")
  let l:lines = s:screen()
  let l:marker = s:row(l:lines, 'MARKER-LINE')
  call s:check(l:marker ==# '' ? '' : s:numbered(l:marker), 1,
        \ 'detector: <C-\><C-n> draws line numbers over the shell')

  " Back to Terminal-Job mode, then navigate up with the real chord.
  call term_sendkeys(s:pane, 'i')
  call term_wait(s:pane, 300)
  call term_sendkeys(s:pane, "\<C-k>")
  let l:lines = s:screen()
  let l:top = s:row(l:lines, 'TOP-A')
  let l:marker = s:row(l:lines, 'MARKER-LINE')
  call s:check(l:top ==# '' ? '' : s:numbered(l:top), 1,
        \ 'chord reached the text window (still numbered)')
  call s:check(l:marker ==# '' ? '' : s:numbered(l:marker), 0,
        \ 'chord left the shell gutter-free (no line numbers over the terminal)')
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

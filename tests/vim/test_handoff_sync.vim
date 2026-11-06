" The edge hand-off is synchronous — no pty, no server.
"
" Run headless:
"   vim -u NONE -N --not-a-term -S tests/vim/test_handoff_sync.vim </dev/null
"
" Why it matters: the helper lives for milliseconds (~3-15 ms for the Rust
" build), and running it synchronously is what makes the chord deterministic —
" when herdr_splits#nav() returns, the focus has already moved, so a second
" keypress cannot race a hand-off that has not happened yet. Vim has no blocking
" wait for jobs (job_status() is poll-only; jobwait()/job_wait() are Neovim
" APIs), so the sync path is system({list}) and the async spawn is only the
" fallback for builds without it.
"
" The stub helper is a .cmd on Windows (a .sh is rejected by the win32 build's
" executable check: E475) and a .sh elsewhere. Marker files prove that the child
" had already run when the call returned.

let s:root = expand('<sfile>:p:h:h:h')

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_handoff_sync: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

execute 'set runtimepath^=' . fnameescape(s:root)
let $HERDR_ENV = '1'
execute 'source' fnameescape(s:root . '/plugin/herdr-splits.vim')

let s:dir = fnamemodify(tempname(), ':h')
let s:marker = s:dir . '/handoff_marker.txt'
let s:exe = has('win32') ? '.cmd' : '.sh'

function! s:stub(name, status) abort
  let l:path = s:dir . '/' . a:name . s:exe
  if has('win32')
    call writefile([
          \ '@echo off',
          \ 'echo ran > "' . s:marker . '"',
          \ 'exit /b ' . a:status,
          \ ], l:path)
  else
    call writefile([
          \ '#!/bin/sh',
          \ 'echo ran > "' . s:marker . '"',
          \ 'exit ' . a:status,
          \ ], l:path)
    call setfperm(l:path, 'rwxr-xr-x')
  endif
  return l:path
endfunction

" One window means every direction is the editor edge, so the chord hands off.
" The stub ignores the arguments (nav --pane-only ... <direction>).

" --- a clean run: the child has finished by the time the chord returns -----
" The chord is fed through the real mapping via :normal, so v:count1 is what a
" keypress gives. (A direct function call from a script sees v:count1 == 0 and,
" by design, no hand-off; feedkeys() at script level does not resolve
" normal-mode mappings either — :normal does.)
call delete(s:marker)
let g:herdr_splits_bin = s:stub('handoff_ok', 0)
execute "normal \<C-h>"
call s:check(filereadable(s:marker), 1, 'helper ran before the chord returned')

" --- it runs on every chord, not just the first ---------------------------
call delete(s:marker)
execute "normal \<C-j>"
call s:check(filereadable(s:marker), 1, 'helper runs on every chord')

" --- a non-zero exit is reported (once) and does not raise ----------------
call delete(s:marker)
let g:herdr_splits_bin = s:stub('handoff_fail', 3)
messages clear
execute "normal \<C-h>"
redir => s:msgs
silent messages
redir END
call s:check(filereadable(s:marker), 1, 'failing helper still ran')
call s:check(s:msgs =~# 'edge hand-off exited with 3', 1,
      \ 'non-zero exit reported')

" --- a missing helper never breaks the chord -----------------------------
" On the win32 build system({list}) raises E475 for a non-executable path; the
" fallback spawn must swallow that instead of aborting the mapping.
let g:herdr_splits_bin = 'no-such-herdr-splits-binary'
messages clear
execute "normal \<C-h>"
redir => s:msgs
silent messages
redir END
call s:check(s:msgs =~# 'E475', 0, 'a missing helper raises no error')
call s:check(s:msgs =~# 'Error detected', 0, 'a missing helper aborts nothing')

call delete(s:marker)
for s:f in [s:dir . '/handoff_ok' . s:exe, s:dir . '/handoff_fail' . s:exe]
  call delete(s:f)
endfor

qa!

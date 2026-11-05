" Terminal-mode navigation contract for plugin/herdr-splits.vim.
"
" Run headless:
"   vim -u NONE -N --not-a-term -S tests/vim/test_terminal_nav.vim </dev/null
"
" Inside a Herdr pane the plugin owns C-hjkl in normal *and* terminal mode (in
" terminal mode it is the only way to leave a split). g:herdr_splits_terminal_nav
" is an install-time opt-out for terminal mode only: normal mode keeps working,
" and no t-mode mapping is defined, so a user's own buffer-local tnoremap wins.

let s:root = expand('<sfile>:p:h:h:h')
let s:plugin = s:root . '/plugin/herdr-splits.vim'

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_terminal_nav: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

" 1 = defined, 0 = absent, for buffer-local 0 (global) mappings.
function! s:mapped(mode, lhs) abort
  return maparg(a:lhs, a:mode, 0, 1) == {} ? 0 : 1
endfunction

function! s:reset() abort
  unlet! g:loaded_herdr_splits
  unlet! g:herdr_splits_terminal_nav
  for s:k in ['h', 'j', 'k', 'l']
    silent! execute 'nunmap <C-' . s:k . '>'
    silent! execute 'tunmap <C-' . s:k . '>'
  endfor
endfunction

" --- default: both modes mapped inside Herdr --------------------------------
call s:reset()
let $HERDR_ENV = '1'
execute 'source ' . fnameescape(s:plugin)
call s:check(g:herdr_splits_terminal_nav, 1, 'switch defaults to on')
for s:k in ['h', 'j', 'k', 'l']
  call s:check(s:mapped('n', '<C-' . s:k . '>'), 1, 'normal <C-' . s:k . '>')
  call s:check(s:mapped('t', '<C-' . s:k . '>'), 1, 'terminal <C-' . s:k . '>')
endfor
call s:check(
      \ maparg('<C-j>', 't'),
      \ '<C-\><C-N>:<C-U>call herdr_splits#nav(''j'', 1)<CR>',
      \ 'terminal mapping routes through herdr_splits#nav')

" --- opt-out: terminal mode untouched, normal mode intact ------------------
call s:reset()
let $HERDR_ENV = '1'
let g:herdr_splits_terminal_nav = 0
execute 'source ' . fnameescape(s:plugin)
for s:k in ['h', 'j', 'k', 'l']
  call s:check(s:mapped('n', '<C-' . s:k . '>'), 1, 'opt-out keeps normal <C-' . s:k . '>')
  call s:check(s:mapped('t', '<C-' . s:k . '>'), 0, 'opt-out drops terminal <C-' . s:k . '>')
endfor

" --- opt-out does not hijack a user's own buffer-local t-mode mapping ------
" (install-time opt-out: no t-mode map is defined at all, so any later
" buffer-local tnoremap the user or a terminal plugin defines is the one that
" applies. Asserted without opening a pty on purpose — see test_nav_term_mode.)
tnoremap <buffer> <C-h> <C-h>
call s:check(maparg('<C-h>', 't'), '<C-H>', 'user buffer-local tnoremap is installable')
silent! tunmap <buffer> <C-h>

" --- outside Herdr: neither mode is mapped --------------------------------
call s:reset()
let $HERDR_ENV = ''
execute 'source ' . fnameescape(s:plugin)
for s:k in ['h', 'j', 'k', 'l']
  call s:check(s:mapped('n', '<C-' . s:k . '>'), 0, 'outside Herdr: normal <C-' . s:k . '>')
  call s:check(s:mapped('t', '<C-' . s:k . '>'), 0, 'outside Herdr: terminal <C-' . s:k . '>')
endfor

qa!

" Terminal-mode navigation contract for plugin/herdr-splits.vim.
"
" Run headless:
"   vim -u NONE -N --not-a-term -S tests/vim/test_terminal_nav.vim </dev/null
"
" Inside a Herdr pane the plugin owns C-hjkl in normal *and* terminal mode (in
" terminal mode it is the only way to leave a split). g:herdr_splits_terminal_nav
" is an install-time opt-out for terminal mode only: normal mode keeps working,
" and no t-mode mapping is defined, so a user's own buffer-local tnoremap wins.
"
" In terminal mode the chord borrows the CTRL-W prefix — `CTRL-W :` runs one Ex
" command and returns to Terminal-Job mode (:h t_CTRL-W_:) — instead of leaving
" Job mode with <C-\><C-n>, which would park the terminal in Terminal-Normal mode
" (line numbers over the shell scrollback) as a side effect of navigating. The
" RHS is therefore an <expr> mapping: the prefix ('termwinkey') and the
" per-filetype skip list are window/buffer state, read when the key fires.

let s:root = expand('<sfile>:p:h:h:h')
let s:plugin = s:root . '/plugin/herdr-splits.vim'

" herdr_splits#term_nav() lives in autoload/, so the checkout has to be on the
" runtimepath — sourcing the plugin file alone does not make it findable.
execute 'set runtimepath^=' . fnameescape(s:root)

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_terminal_nav: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

" Mapping dictionary, or {} when absent.
function! s:map(mode, lhs) abort
  return maparg(a:lhs, a:mode, 0, 1)
endfunction

function! s:mapped(mode, lhs) abort
  return s:map(a:mode, a:lhs) == {} ? 0 : 1
endfunction

function! s:has_nav(chord) abort
  return stridx(a:chord, 'herdr_splits#nav(') >= 0 ? 1 : 0
endfunction

function! s:reset() abort
  unlet! g:loaded_herdr_splits
  unlet! g:herdr_splits_terminal_nav
  unlet! g:herdr_splits_terminal_skip_filetypes
  for s:k in ['h', 'j', 'k', 'l']
    silent! execute 'nunmap <C-' . s:k . '>'
    silent! execute 'tunmap <C-' . s:k . '>'
  endfor
  setlocal filetype=
  setlocal termwinkey=
endfunction

" --- default: both modes mapped inside Herdr --------------------------------
call s:reset()
let $HERDR_ENV = '1'
execute 'source' fnameescape(s:plugin)
call s:check(g:herdr_splits_terminal_nav, 1, 'switch defaults to on')
call s:check(g:herdr_splits_terminal_skip_filetypes, ['fzf'],
      \ 'skip list defaults to fzf')
for s:k in ['h', 'j', 'k', 'l']
  call s:check(s:mapped('n', '<C-' . s:k . '>'), 1, 'normal <C-' . s:k . '>')
  call s:check(s:mapped('t', '<C-' . s:k . '>'), 1, 'terminal <C-' . s:k . '>')
  let s:t = s:map('t', '<C-' . s:k . '>')
  call s:check(get(s:t, 'expr', 0), 1, 'terminal <C-' . s:k . '> is <expr>')
  call s:check(s:t.rhs, "herdr_splits#term_nav('" . s:k . "')",
        \ 'terminal <C-' . s:k . '> routes through herdr_splits#term_nav')
endfor

" --- the chord itself: borrowed prefix, never a mode switch ----------------
let s:chord = herdr_splits#term_nav('j')
call s:check(strpart(s:chord, 0, 1), "\<C-W>", 'chord starts with CTRL-W')
call s:check(strpart(s:chord, 1, 1), ':', 'prefix is followed by :')
call s:check(stridx(s:chord, "\<C-\>\<C-n>"), -1, 'chord sends no <C-\><C-n>')
call s:check(s:has_nav(s:chord), 1, 'chord calls herdr_splits#nav')
call s:check(strpart(s:chord, strlen(s:chord) - 1), "\<CR>", 'chord ends with <CR>')
call s:check(stridx(s:chord, "nav('j')") > 0, 1, 'chord carries the direction')

" --- skip list: the program inside keeps the chord ------------------------
setlocal filetype=fzf
call s:check(herdr_splits#term_nav('j'), "\<C-j>", 'fzf buffer keeps <C-j>')
call s:check(herdr_splits#term_nav('l'), "\<C-l>", 'fzf buffer keeps <C-l>')
setlocal filetype=
call s:check(s:has_nav(herdr_splits#term_nav('j')), 1, 'plain buffer navigates')
let g:herdr_splits_terminal_skip_filetypes = []
setlocal filetype=fzf
call s:check(s:has_nav(herdr_splits#term_nav('j')), 1,
      \ 'empty skip list maps fzf too')
let g:herdr_splits_terminal_skip_filetypes = ['fzf', 'lazygit']
setlocal filetype=lazygit
call s:check(s:has_nav(herdr_splits#term_nav('j')), 0, 'second entry is skipped')
call s:check(g:herdr_splits_terminal_skip_filetypes, ['fzf', 'lazygit'],
      \ 'a user list is not rewritten by the plugin')
unlet g:herdr_splits_terminal_skip_filetypes
setlocal filetype=

" --- the prefix follows 'termwinkey' --------------------------------------
" The option stores what was written (`<C-L>`, `^I`, a letter), not the key.
setlocal termwinkey=<C-L>
call s:check(strpart(herdr_splits#term_nav('j'), 0, 1), "\<C-L>",
      \ 'chord honors termwinkey=<C-L>')
setlocal termwinkey=^I
call s:check(strpart(herdr_splits#term_nav('j'), 0, 1), "\<Tab>",
      \ 'chord honors caret notation')
setlocal termwinkey=X
call s:check(strpart(herdr_splits#term_nav('j'), 0, 1), 'X',
      \ 'chord honors a letter termwinkey')
setlocal termwinkey=
call s:check(strpart(herdr_splits#term_nav('j'), 0, 1), "\<C-W>",
      \ 'empty termwinkey means CTRL-W')

" --- every direction maps to its own chord --------------------------------
setlocal filetype=
for s:k in ['h', 'j', 'k', 'l']
  call s:check(s:has_nav(herdr_splits#term_nav(s:k)), 1, 'chord for ' . s:k)
  call s:check(stridx(herdr_splits#term_nav(s:k), "nav('" . s:k . "')") > 0, 1,
        \ 'chord for ' . s:k . ' carries the direction')
endfor

" --- the old two-argument entry point still runs --------------------------
" Mappings copied from older docs pass the removed from_term flag.
let s:navless = 0
try
  call herdr_splits#nav('j', 1)
catch
  let s:navless = 1
endtry
call s:check(s:navless, 0, 'herdr_splits#nav(key, 1) still accepted')

" --- opt-out: terminal mode untouched, normal mode intact ------------------
call s:reset()
let $HERDR_ENV = '1'
let g:herdr_splits_terminal_nav = 0
execute 'source' fnameescape(s:plugin)
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
execute 'source' fnameescape(s:plugin)
for s:k in ['h', 'j', 'k', 'l']
  call s:check(s:mapped('n', '<C-' . s:k . '>'), 0, 'outside Herdr: normal <C-' . s:k . '>')
  call s:check(s:mapped('t', '<C-' . s:k . '>'), 0, 'outside Herdr: terminal <C-' . s:k . '>')
endfor

qa!

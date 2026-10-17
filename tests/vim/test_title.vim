" Title marker contract for plugin/herdr-splits.vim.
"
" Run headless:
"   vim -u NONE -N --not-a-term -S tests/vim/test_title.vim </dev/null
"
" The marker is how plain Vim advertises the plugin to Herdr; it must be
" appended to a custom titlestring, supplied as a sane default when the user
" has none, and left alone outside a Herdr pane.

let s:root = expand('<sfile>:p:h:h:h')
let s:plugin = s:root . '/plugin/herdr-splits.vim'

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_title: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

let $HERDR_ENV = '1'
let &titlestring = 'CUSTOM'
execute 'source ' . fnameescape(s:plugin)
call s:check(&titlestring, 'CUSTOM [herdr-splits]', 'custom titlestring')
call s:check(string(&title), '1', 'title enabled inside Herdr')

" Re-source from a clean guard for the default-title case.
unlet g:loaded_herdr_splits
set titlestring=
execute 'source ' . fnameescape(s:plugin)
call s:check(&titlestring, '%t - VIM [herdr-splits]', 'default titlestring')

" Outside Herdr the plugin must not touch the title.
unlet g:loaded_herdr_splits
let $HERDR_ENV = ''
set notitle
let &titlestring = 'OUTSIDE'
execute 'source ' . fnameescape(s:plugin)
call s:check(&titlestring, 'OUTSIDE', 'outside Herdr')
call s:check(string(&title), '0', 'title option untouched outside Herdr')

qa!

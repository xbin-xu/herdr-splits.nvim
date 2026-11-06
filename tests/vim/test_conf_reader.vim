" Reader parity for the shared conf on the plain-Vim side — no pty, no server.
"
" Run headless:
"   vim -u NONE -N --not-a-term -S tests/vim/test_conf_reader.vim </dev/null
"
" Vim only reads the instance policy (it has no writer at all): the MRU switch
" comes from the conf unless the user overrides it with a g: variable. The
" fallback when the file or the key is absent must be the shipped default —
" `most_recently_focused=true` — because that is what the Rust helper and Neovim
" use, and what `herdr-splits init` writes.

let s:root = expand('<sfile>:p:h:h:h')

function! s:check(got, want, label) abort
  if a:got !=# a:want
    echomsg 'test_conf_reader: ' . a:label . ': got ' . string(a:got)
          \ . ', want ' . string(a:want)
    cquit 1
  endif
endfunction

execute 'set runtimepath^=' . fnameescape(s:root)
let $HERDR_ENV = '1'
runtime autoload/herdr_splits.vim

let s:sid = 0
for s:info in getscriptinfo()
  if s:info.name =~# 'autoload[/\\]herdr_splits\.vim$'
    let s:sid = s:info.sid
  endif
endfor
call s:check(s:sid > 0, 1, 'autoload script id resolved')

function! s:most_recently_focused() abort
  return call(function('<SNR>' . s:sid . '_most_recently_focused'), [])
endfunction

call s:check(exists('g:herdr_splits_most_recently_focused'), 0, 'no override set')

let s:conf = tempname() . '.conf'
let $HERDR_SPLITS_CONFIG = s:conf

" --- the file is missing: the shipped default applies ---------------------
" (`herdr-splits` creates it on its first run; until then every reader must
" agree on the same value.)
call delete(s:conf)
call s:check(s:most_recently_focused(), 1, 'missing conf -> default on')

" --- written out explicitly, both ways ------------------------------------
call writefile(['most_recently_focused=false'], s:conf)
call s:check(s:most_recently_focused(), 0, 'conf says false')

call writefile(['most_recently_focused=true'], s:conf)
call s:check(s:most_recently_focused(), 1, 'conf says true')

" --- last occurrence wins (the Rust reader takes the last one) ------------
call writefile(['most_recently_focused=true', 'most_recently_focused = false  # later wins'], s:conf)
call s:check(s:most_recently_focused(), 0, 'last value wins')

" --- comments and other keys do not count ---------------------------------
call writefile(['# most_recently_focused=true', 'unzoom_on_nav=false'], s:conf)
call s:check(s:most_recently_focused(), 1, 'commented key ignored -> default on')

" --- a generated file reads the same --------------------------------------
call writefile([
      \ '# herdr-splits-generated-v1',
      \ '# Written because setup({ overwrite_conf = true }): this file is a projection',
      \ '# of the Lua options — remove an opt in setup() to revert it.',
      \ 'nav_at_edge=wrap',
      \ 'most_recently_focused=false',
      \ ], s:conf)
call s:check(s:most_recently_focused(), 0, 'generated file parsed')

" --- a g: override still wins over the conf ------------------------------
call writefile(['most_recently_focused=false'], s:conf)
let g:herdr_splits_most_recently_focused = 1
call s:check(s:most_recently_focused(), 1, 'g: override beats the conf')
unlet g:herdr_splits_most_recently_focused

" --- case-insensitive, like the Rust/Python/Lua readers ------------------
call writefile(['most_recently_focused=TRUE'], s:conf)
call s:check(s:most_recently_focused(), 1, 'TRUE counts as true')

call writefile(['most_recently_focused=FALSE'], s:conf)
call s:check(s:most_recently_focused(), 0, 'FALSE counts as false')

call delete(s:conf)
echomsg 'test_conf_reader: OK'

qall!

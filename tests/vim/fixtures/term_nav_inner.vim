" Inner session for tests/vim/test_term_nav_render.vim.
"
" Runs inside a nested Vim's pty: a numbered text buffer on top, a shell that
" printed MARKER-LINE at the bottom, cursor left in the terminal so the session
" starts in Terminal-Job mode. The outer test sends real keys through the pty
" and reads the drawn screen back with term_getline().
"
" $HERDR_SPLITS_TEST_ROOT is set by the test (the checkout under test).

set nocompatible
set number
set noswapfile
set laststatus=0
set shortmess+=I

execute 'set runtimepath^=' . fnameescape($HERDR_SPLITS_TEST_ROOT)
let $HERDR_ENV = '1'
execute 'source' fnameescape($HERDR_SPLITS_TEST_ROOT . '/plugin/herdr-splits.vim')

enew
call setline(1, ['TOP-A', 'TOP-B', 'TOP-C'])
belowright split
let s:job = term_start([&shell, &shellcmdflag, 'echo MARKER-LINE; sleep 30'],
      \ {'term_rows': 6, 'curwin': 1})
call term_wait(s:job, 800)

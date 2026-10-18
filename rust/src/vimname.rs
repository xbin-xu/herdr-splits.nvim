//! The vim-family process-name matcher.
//!
//! The original Python used the regular expression
//!
//! ```text
//! ^(g?(view|l?n?vim?x?)(diff)?)(\.exe)?$
//! ```
//!
//! `import re` cost ~15 ms of every keypress on Windows, so the slim rewrite
//! replaced it with a hand-rolled string check; this is a port of that check.
//! It is deliberately a *full match*: `notvim`, `myvim` and `vim.` are not vim.
//!
//! Note on the family's shape: the core alternation is `view` *or*
//! `l?n?vim?x?`, so `gnview` and `nview` are *not* vim (the `g`/`l`/`n` prefixes
//! belong to the `vim` branch, not to `view`), while `gnvim`, `lnvimdiff` and
//! `nvimx` are. `mvim` (MacVim) does not match either. All of that is
//! preserved bug-for-bug, and pinned by `tests/vimnames.tsv`, a truth table
//! generated from the original Python function.

/// True when `name` is the name of a vim-family process, with an optional
/// `.exe` suffix and case-insensitive comparison.
///
/// Stripping order matters: the optional `g` and `diff` wrap the core, so both
/// are peeled before the core is matched.
pub fn is_vim_process_name(name: &str) -> bool {
    let mut lowered = name.to_lowercase();
    if let Some(stripped) = lowered.strip_suffix(".exe") {
        lowered.truncate(stripped.len());
    }
    if let Some(stripped) = lowered.strip_prefix('g') {
        lowered = stripped.to_string();
    }
    if let Some(stripped) = lowered.strip_suffix("diff") {
        lowered.truncate(stripped.len());
    }
    if lowered == "view" {
        return true;
    }
    if let Some(stripped) = lowered.strip_prefix('l') {
        lowered = stripped.to_string();
    }
    if let Some(stripped) = lowered.strip_prefix('n') {
        lowered = stripped.to_string();
    }
    if let Some(stripped) = lowered.strip_prefix("vi") {
        lowered = stripped.to_string();
    } else {
        return false;
    }
    if let Some(stripped) = lowered.strip_prefix('m') {
        lowered = stripped.to_string();
    }
    if let Some(stripped) = lowered.strip_prefix('x') {
        lowered = stripped.to_string();
    }
    lowered.is_empty()
}

#[cfg(test)]
mod tests {
    use super::is_vim_process_name;

    #[test]
    fn accepts_the_whole_vim_family() {
        // Values checked against the original Python regex.
        for name in [
            "vim",
            "vim.exe",
            "vi",
            "vi.exe",
            "view",
            "view.exe",
            "vimx",
            "vimx.exe",
            "vix",
            "vix.exe",
            "gview",
            "gview.exe",
            "gvim",
            "gvim.exe",
            "gvi",
            "gvi.exe",
            "nvim",
            "nvim.exe",
            "nvi",
            "nvi.exe",
            "nvix",
            "nvimx",
            "lnvim",
            "lnvim.exe",
            "lvim",
            "lvi.exe",
            "nvimdiff",
            "vimdiff",
            "vimdiff.exe",
            "gvimdiff",
            "gvimdiff.exe",
            "gnvim",
            "gnvim.exe",
            "glnvim",
            "lnvimdiff",
            "VIM",
            "Vim.Exe",
            "NVIM.EXE",
            "GVIMDIFF.EXE",
            "GVI",
        ] {
            assert!(is_vim_process_name(name), "{name} should match");
        }
    }

    #[test]
    fn rejects_everything_else() {
        // `gnview`/`nview` are the surprising ones: the core alternation is
        // `view` or `l?n?vim?x?`, so a prefix before `view` cannot match.
        for name in [
            "",
            "bash",
            "bash.exe",
            "node.exe",
            "sh",
            "notvim",
            "myvim",
            "vim.",
            "vimx.exe.exe",
            "gnview",
            "nview",
            "nview.exe",
            "nvim2",
            "vimrc",
            "view.exe.bak",
            "mvim",
            "mvim.exe",
            "ldiff",
            "lgvvim",
            "gvimrc",
            "vim_x",
            "vi.exe.exe",
            "ggvim",
            "nview.exe",
            "nvim_x",
        ] {
            assert!(!is_vim_process_name(name), "{name} should not match");
        }
    }

    #[test]
    fn reports_nvim_panes_as_nvim() {
        // `pane_editor` distinguishes the two with a substring test on the
        // name, so the two sets must not intersect.
        for name in ["nvim", "nvim.exe", "NVIM.EXE", "gnvim", "lnvim", "nvimx"] {
            assert!(
                is_vim_process_name(name) && name.to_lowercase().contains("nvim"),
                "{name} should be nvim"
            );
        }
        for name in ["vim", "vim.exe", "gvim", "view", "vimdiff"] {
            assert!(
                is_vim_process_name(name) && !name.to_lowercase().contains("nvim"),
                "{name} should be plain vim"
            );
        }
        // `lnvim` matches both "is vim" and "contains nvim" — it is reported as
        // nvim, which is correct: it is an nvim wrapper, not plain vim.
        assert!(is_vim_process_name("lnvim") && "lnvim".contains("nvim"));
    }

    #[test]
    fn matches_the_python_truth_table() {
        // Generated from the original Python matcher (exhaustive over the
        // alphabet "vilnmgxd" up to length 3, plus .exe and uppercase
        // variants) so the Rust port cannot drift from the fallback silently.
        let table = include_str!("../tests/vimnames.tsv");
        let mut checked = 0;
        let mut accepted = 0;
        for line in table.lines() {
            let Some((name, expected)) = line.rsplit_once('\t') else {
                continue;
            };
            let expected = expected == "1";
            let actual = is_vim_process_name(name);
            assert_eq!(actual, expected, "{name:?}");
            checked += 1;
            if expected {
                accepted += 1;
            }
        }
        assert!(checked > 1000, "fixture looks truncated: {checked}");
        assert!(accepted > 10, "fixture has no positive cases: {accepted}");
    }
}

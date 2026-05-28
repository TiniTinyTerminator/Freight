# TUI TODO

## Scope

TUI is only for commands where interactive selection genuinely helps the user:

| Command | Status | Notes |
|---|---|---|
| `freight add` | ✓ Done | Search, browse, select version, add to freight.toml |
| `freight login` | ✓ Done | Password form, saves token |
| `freight register` | ✓ Done | Registration form |

All other commands (`build`, `test`, `tui`, etc.) output to the normal terminal.
The `tui/registry/` directory contains an admin panel implementation that is
currently not exposed — kept for reference.

## `freight add` improvements

- [x] tui-markdown README rendering (wide layout, middle column)
- [x] 3-column layout at ≥ 100 cols: package list | README | info + versions
- [x] Info pane: name, latest version, description, dep count
- [x] Info pane: Tags (keywords) + Owners — fetched alongside package detail
- [x] Info pane height = 50% of right column; description truncated to fit
- [x] Versions panel tab toggle: Versions | Dependencies (`t` key)
- [x] Dependencies tab: shows `dep  @version` for selected version
- [ ] Show package license in the Info pane (needs registry API field to be wired)

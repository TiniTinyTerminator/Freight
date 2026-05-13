# Ideas

## Package lookup improvements
Freight now lets users declare version-only dependencies and resolves them by
checking system metadata first, then falling back to vcpkg. The longer-term lookup
can still become richer as additional repositories and ownership databases are added:

```text
logical dependency
    ↓
system metadata (pkg-config / cmake metadata)
    ↓
package manager ownership lookup
    ↓
repository fallback (vcpkg today, more later)
    ↓
raw probing fallback
```

When multiple repositories exist, add an explicit selector such as:

```toml
zlib = { version = "1.3", repo = "vcpkg" }
```

## System cache registry
Whenever new packages are installed, update an internal registry with the new packages. Only stores libraries and headers.

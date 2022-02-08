# startup-db

`sqlx` does not support tracing by default.
You can either use a patched `sqlx-core` or annotate your `sqlx` calls manually:
```toml
# add tracing support
[patch."crates-io".sqlx-core]
git = "https://github.com/flachnetz/sqlx.git"
branch = "v0.5.10-tracing"
```

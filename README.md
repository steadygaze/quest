# Quest

## Development

### Setup

```shell
cargo install cargo-watch systemfd
```

### Running

You can simply run `cargo run` and access the site at the default location (`localhost:8080`). However, if you want live reloading, you can do the following:

1. In one terminal, run:

   ```shell
   systemfd --no-pid -s http::8080 -- cargo watch -x run
   ```

2. In another terminal, run:

   ```shell
   npx browser-sync start --proxy "localhost:8080" --reload-delay 100 --files "src" "static" "tailwind.config.js" "templates"
   ```

3. Open the site at the port provided by Browsersync (likely `3000`), rather than the unproxied application port (`8080`).


# Quest

## Development

### Setup

```shell
cargo install systemfd
```

### Running

In one terminal, run:

```shell
systemfd --no-pid -s http::8080 -- cargo watch -x run
```

In another terminal, run:

```shell
npx browser-sync start --proxy "localhost:8080" --files "src" "static" "tailwind.config.js" "templates" --reload-delay 100
```

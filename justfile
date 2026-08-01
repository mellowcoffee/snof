set default-list

fmt:
    cargo +nightly fmt 
test:
    cargo test
check: fmt test
    cargo clippy

watch:
    cargo watch -cq -x clippy

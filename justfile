set default-list

fmt:
    cargo +nightly fmt 
check: fmt
    cargo clippy

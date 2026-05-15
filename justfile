set shell := ["sh", "-eu", "-c"]


default:
    just --list

install-devtools:
    cargo install cargo-llvm-cov --version 0.6.13 --locked

quick_check:
    clear
    cargo check --workspace --bins --message-format=short


coverage:
    cargo llvm-cov clean --workspace
    RUSTFLAGS="-C opt-level=0" cargo llvm-cov \
      --workspace \
      --html \
      --fail-under-lines 95 \

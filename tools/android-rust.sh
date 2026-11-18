# Source this with task_root set to the project's absolute path.
if [[ -n "${SMASH_RUST_TOOLCHAIN:-}" ]]; then
  if [[ ! -x "$SMASH_RUST_TOOLCHAIN/bin/cargo" || ! -x "$SMASH_RUST_TOOLCHAIN/bin/rustc" ]]; then
    echo 'SMASH_RUST_TOOLCHAIN must contain bin/cargo and bin/rustc.' >&2
    exit 1
  fi
  export PATH="$SMASH_RUST_TOOLCHAIN/bin:$PATH"
elif [[ -x "$task_root/.tools/rust/bin/cargo" && -x "$task_root/.tools/rust/bin/rustc" ]]; then
  export PATH="$task_root/.tools/rust/bin:$PATH"
elif [[ -d "$HOME/.cargo/bin" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if ! command -v cargo >/dev/null || ! command -v rustc >/dev/null; then
  echo 'Install Rust with Rustup or set SMASH_RUST_TOOLCHAIN.' >&2
  exit 1
fi
if [[ -d "$task_root/.tools/cargo" ]]; then
  export CARGO_HOME="${CARGO_HOME:-$task_root/.tools/cargo}"
else
  export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
fi

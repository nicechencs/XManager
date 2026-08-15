#!/usr/bin/env bash
# XManager launcher — run from repo root
# Examples:
#   ./run.sh              release build + run
#   ./run.sh debug        debug build + run
#   ./run.sh bin          run existing release binary only (no rebuild)
#   ./run.sh debug bin    run existing debug binary only
#   ./run.sh cli -- --help

set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT" || exit 1

MODE=release
BIN_ONLY=0
PASS_ARGS=()

usage() {
  cat <<'EOF'
Usage: ./run.sh [release|debug] [bin] [-- app-args...]
       ./run.sh cli [-- cli-args...]

  release   Build and run release binary (default)
  debug     Build and run debug binary
  bin       Skip cargo; run existing binary only
  cli       Run xmanager-cli (JSON automation)
  help      Show this help

Examples:
  ./run.sh
  ./run.sh debug
  ./run.sh release bin
  ./run.sh cli -- --help
  ./run.sh cli -- filter --input tweets.json --max-views 20
EOF
}

os_name() {
  uname -s 2>/dev/null || echo unknown
}

binary_path() {
  local mode="$1"
  local bin
  if [[ "$mode" == release ]]; then
    bin="target/release/xmanager"
  else
    bin="target/debug/xmanager"
  fi
  if [[ ! -e "$bin" && -e "${bin}.exe" ]]; then
    bin="${bin}.exe"
  fi
  printf '%s\n' "$bin"
}

warn_if_no_env() {
  if [[ ! -f .env ]]; then
    echo "[warn] .env not found. Copy .env.example to .env and fill OAuth credentials."
    echo
  fi
}

require_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "[error] cargo not found. Install Rust from https://rustup.rs"
    exit 1
  fi
}

warn_linux_desktop() {
  [[ "$(os_name)" == Linux ]] || return 0

  if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
    echo "[warn] No WAYLAND_DISPLAY or DISPLAY. The desktop window needs a graphical session."
    echo "       Use ./run.sh cli for headless automation."
    echo
  fi

  local missing=()
  local lib
  for lib in libvulkan.so.1 libxkbcommon.so.0; do
    if ! ldconfig -p 2>/dev/null | grep -Fq "$lib"; then
      if [[ ! -e "/usr/lib/x86_64-linux-gnu/$lib" && ! -e "/usr/lib/$lib" && ! -e "/usr/lib64/$lib" ]]; then
        missing+=("$lib")
      fi
    fi
  done
  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "[warn] Missing Linux libraries for the GPUI desktop app: ${missing[*]}"
    echo "       Debian/Ubuntu: sudo apt install -y libvulkan1 libxkbcommon0 libwayland-client0 mesa-vulkan-drivers"
    echo "       Build headers: sudo apt install -y clang pkg-config libxkbcommon-dev libwayland-dev libvulkan-dev libfontconfig-dev"
    echo
  fi
}

run_cli() {
  require_cargo
  if [[ ${#PASS_ARGS[@]} -gt 0 && "${PASS_ARGS[0]}" == "--" ]]; then
    PASS_ARGS=("${PASS_ARGS[@]:1}")
  fi
  echo "[info] Running xmanager-cli..."
  cargo run -p xmanager-cli -- "${PASS_ARGS[@]}"
  local ec=$?
  if [[ "$ec" -ne 0 ]]; then
    echo "[error] xmanager-cli exited with code $ec"
    exit "$ec"
  fi
  exit 0
}

run_existing_bin() {
  local exe
  exe="$(binary_path "$MODE")"
  if [[ ! -e "$exe" ]]; then
    echo "[error] Binary not found: $exe"
    echo "        Build first with: ./run.sh $MODE"
    exit 1
  fi
  if [[ ! -x "$exe" ]]; then
    chmod +x "$exe" 2>/dev/null || true
  fi
  warn_linux_desktop
  echo "[info] Launching $exe ..."
  "$exe" "${PASS_ARGS[@]}"
  local ec=$?
  if [[ "$ec" -ne 0 ]]; then
    echo "[error] XManager exited with code $ec"
    exit "$ec"
  fi
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    help|-h|--help)
      usage
      exit 0
      ;;
    release)
      MODE=release
      shift
      ;;
    debug)
      MODE=debug
      shift
      ;;
    bin)
      BIN_ONLY=1
      shift
      ;;
    cli)
      shift
      PASS_ARGS=("$@")
      run_cli
      ;;
    --)
      shift
      PASS_ARGS+=("$@")
      break
      ;;
    *)
      PASS_ARGS+=("$1")
      shift
      ;;
  esac
done

warn_if_no_env

if [[ "$BIN_ONLY" -eq 1 ]]; then
  run_existing_bin
fi

require_cargo
warn_linux_desktop

echo "[info] Building xmanager-ui ($MODE)..."
if [[ "$MODE" == release ]]; then
  cargo run -p xmanager-ui --release -- "${PASS_ARGS[@]}"
else
  cargo run -p xmanager-ui -- "${PASS_ARGS[@]}"
fi
ec=$?
if [[ "$ec" -ne 0 ]]; then
  echo "[error] XManager exited with code $ec"
  exit "$ec"
fi
exit 0

#!/usr/bin/env bash
# Shared colors and visual feedback for scripts/ci/*.
# Source from any job script. Safe under `set -euo pipefail`.

if [[ -n "${_CI_HELPERS_LOADED:-}" ]]; then
  return 0 2>/dev/null || exit 0
fi
_CI_HELPERS_LOADED=1

_CI_HELPERS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${_CI_HELPERS_DIR}/.." && pwd)"
export REPO_ROOT

_ci_flag_on() {
  local v="${1:-}"
  [[ -n "$v" && "$v" != "0" && "$v" != "false" ]]
}

_ci_use_color() {
  _ci_flag_on "${FORCE_COLOR:-}" && return 0
  _ci_flag_on "${CLICOLOR_FORCE:-}" && return 0
  _ci_flag_on "${GITHUB_ACTIONS:-}" && return 0
  _ci_flag_on "${NO_COLOR:-}" && return 1
  [[ -t 1 ]] && return 0
  return 1
}

if _ci_use_color; then
  C_RESET=$'\033[0m'
  C_BOLD=$'\033[1m'
  C_DIM=$'\033[2m'
  C_RED=$'\033[31m'
  C_GREEN=$'\033[32m'
  C_YELLOW=$'\033[33m'
  C_BLUE=$'\033[34m'
  C_CYAN=$'\033[36m'
else
  C_RESET="" C_BOLD="" C_DIM="" C_RED="" C_GREEN="" C_YELLOW="" C_BLUE="" C_CYAN=""
fi

_ci_ts() { date -u "+%H:%M:%S"; }

log_step() {
  printf "%s%s[%s] ▶ %s%s%s\n" "$C_BOLD" "$C_CYAN" "$(_ci_ts)" "$*" "$C_RESET" >&2
}

log_info() {
  printf "%s[%s] i %s%s\n" "$C_BLUE" "$(_ci_ts)" "$*" "$C_RESET" >&2
}

log_ok() {
  printf "%s[%s] ok %s%s\n" "$C_GREEN" "$(_ci_ts)" "$*" "$C_RESET" >&2
}

log_warn() {
  printf "%s[%s] warn %s%s\n" "$C_YELLOW" "$(_ci_ts)" "$*" "$C_RESET" >&2
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "::warning::$*"
  fi
}

log_debug() {
  if [[ "${CI_DEBUG:-}" == "1" ]]; then
    printf "%s[%s] debug %s%s\n" "$C_DIM" "$(_ci_ts)" "$*" "$C_RESET" >&2
  fi
}

log_error() {
  printf "%s%s[%s] error %s%s\n" "$C_BOLD" "$C_RED" "$(_ci_ts)" "$*" "$C_RESET" >&2
}

ci_banner() {
  printf "\n%s%s━━ %s ━━%s\n" "$C_BOLD" "$C_CYAN" "$*" "$C_RESET" >&2
}

ci_die() {
  trap - ERR
  local message="${1:-command failed}"
  log_error "$message"
  log_info "pwd=$(pwd)"
  log_info "repo=${REPO_ROOT}"
  if [[ -n "${GITHUB_JOB:-}" ]]; then
    log_info "github_job=${GITHUB_JOB} actor=${GITHUB_ACTOR:-?} ref=${GITHUB_REF:-?}"
  fi
  if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
    echo "::error::$message"
  fi
  exit 1
}

_ci_err_trap() {
  local ec=$?
  trap - ERR
  ci_die "command failed (exit ${ec}): ${BASH_COMMAND}"
}

set -E
trap '_ci_err_trap' ERR

require_env() {
  local name
  for name in "$@"; do
    if [[ -z "${!name:-}" ]]; then
      ci_die "missing required environment variable: ${name}"
    fi
  done
}

require_cmd() {
  local name
  for name in "$@"; do
    if ! command -v "$name" >/dev/null 2>&1; then
      ci_die "required command not found: ${name}"
    fi
  done
}

ci_output() {
  local key="$1"
  local value="$2"
  if [[ -z "${GITHUB_OUTPUT:-}" ]]; then
    log_debug "GITHUB_OUTPUT unset; ${key}=${value}"
    return 0
  fi
  printf "%s=%s\n" "$key" "$value" >> "$GITHUB_OUTPUT"
  log_debug "output ${key}=${value}"
}

ci_sed_inplace() {
  local expr="$1"
  local file="$2"
  [[ -f "$file" ]] || ci_die "file not found for sed: ${file}"
  if [[ "$(uname -s)" == Darwin ]]; then
    sed -i "" -E "$expr" "$file" || ci_die "sed failed on ${file}"
  else
    sed -i -E "$expr" "$file" || ci_die "sed failed on ${file}"
  fi
}

# Sets TAG (always v-prefixed) and TAG_BARE from a dispatch input with or without v.
ci_parse_tag_input() {
  local raw
  raw="$(printf '%s' "${1:-}" | tr -d '[:space:]')"
  [[ -n "$raw" ]] || ci_die "tag input is empty"
  local bare="$raw"
  if [[ "$bare" == [vV]* ]]; then
    bare="${bare:1}"
  fi
  [[ -n "$bare" ]] || ci_die "tag input is empty after stripping v prefix"
  TAG_BARE="$bare"
  TAG="v${bare}"
}

ci_each_tag_name() {
  [[ -n "${TAG:-}" ]] || ci_die "TAG is unset"
  printf '%s\n' "$TAG"
  if [[ -n "${TAG_BARE:-}" && "$TAG_BARE" != "$TAG" ]]; then
    printf '%s\n' "$TAG_BARE"
  fi
}

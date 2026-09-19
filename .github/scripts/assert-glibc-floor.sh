#!/usr/bin/env bash
# Assert that shared libraries do not require a glibc newer than the given version.

set -euo pipefail

if [ "$#" -lt 2 ]; then
  echo "usage: $0 <max-glibc-version> <file>..." >&2
  exit 2
fi

max="$1"
shift

version_gt() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n1)" = "$1" ]
}

glibc_versions() {
  readelf --dyn-syms -W "$1" | grep -oE '@GLIBC_[0-9.]+' | sed 's/^@GLIBC_//' | sort -uV
}

glibc_symbols_for() {
  readelf --dyn-syms -W "$1" | grep -oE "[A-Za-z_][A-Za-z0-9_]*@GLIBC_${2//./\\.}([^0-9.]|\$)" |
    sed 's/@.*//' | sort -u | tr '\n' ' '
}

status=0
for file in "$@"; do
  if [ ! -f "$file" ]; then
    echo "::error::${file} does not exist"
    status=1
    continue
  fi

  versions="$(glibc_versions "$file" || true)"
  if [ -z "$versions" ]; then
    echo "${file}: no versioned glibc symbols, nothing to check"
    continue
  fi

  floor="$(echo "$versions" | tail -n1)"

  if version_gt "$floor" "$max"; then
    echo "::error::${file} requires glibc ${floor}, newer than the supported floor ${max}. It was most likely built outside the old-glibc container."
    for version in $versions; do
      if version_gt "$version" "$max"; then
        echo "  needs GLIBC_${version}: $(glibc_symbols_for "$file" "$version")"
      fi
    done
    status=1
  else
    echo "${file}: requires at most glibc ${floor} (floor ${max})"
  fi
done

exit "$status"

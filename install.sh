#!/bin/sh
# OPPF installer — install or upgrade the `opp` CLI.
#
#   curl -fsSL https://raw.githubusercontent.com/camelop/oppf/main/install.sh | sh
#
# Idempotent: re-running installs the latest release, replacing any existing
# binary. If the latest version is already installed it exits without changes
# (use --force to reinstall). This same command is how you upgrade.
#
# Options / environment:
#   --version <vX.Y.Z> | OPP_VERSION   pin a specific release (default: latest)
#   --dir <path>       | OPP_INSTALL_DIR  install location (default: ~/.local/bin)
#   --force | -f                        reinstall even if up to date
set -eu

REPO="camelop/oppf"
BIN="opp"
: "${OPP_INSTALL_DIR:=${HOME}/.local/bin}"
VERSION="${OPP_VERSION:-}"
FORCE=0

info() { printf 'opp-install: %s\n' "$1" >&2; }
err() {
	printf 'opp-install: error: %s\n' "$1" >&2
	exit 1
}
have() { command -v "$1" >/dev/null 2>&1; }

usage() {
	cat >&2 <<EOF
Install or upgrade the opp CLI.

Usage: install.sh [--version vX.Y.Z] [--dir PATH] [--force]

  --version, OPP_VERSION       release to install (default: latest)
  --dir,     OPP_INSTALL_DIR   install directory (default: ~/.local/bin)
  --force, -f                  reinstall even if already up to date
EOF
}

while [ $# -gt 0 ]; do
	case "$1" in
	--version)
		VERSION="${2:-}"
		shift 2
		;;
	--version=*)
		VERSION="${1#*=}"
		shift
		;;
	--dir)
		OPP_INSTALL_DIR="${2:-}"
		shift 2
		;;
	--dir=*)
		OPP_INSTALL_DIR="${1#*=}"
		shift
		;;
	--force | -f)
		FORCE=1
		shift
		;;
	-h | --help)
		usage
		exit 0
		;;
	*) err "unknown option: $1 (try --help)" ;;
	esac
done

# --- download helpers -------------------------------------------------------
download() { # <url> <out-file>
	if have curl; then
		curl -fsSL "$1" -o "$2"
	elif have wget; then
		wget -qO "$2" "$1"
	else
		err "need curl or wget to download files"
	fi
}
fetch() { # <url>  -> stdout
	if have curl; then
		curl -fsSL "$1"
	elif have wget; then
		wget -qO- "$1"
	else
		err "need curl or wget to download files"
	fi
}

# --- detect platform --------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
Linux) os="linux" ;;
Darwin) os="darwin" ;;
*) err "unsupported OS: $os (the installer supports Linux and macOS)" ;;
esac
case "$arch" in
x86_64 | amd64) arch="x86_64" ;;
aarch64 | arm64) arch="aarch64" ;;
*) err "unsupported architecture: $arch" ;;
esac
case "$os" in
linux) target="${arch}-unknown-linux-musl" ;;
darwin) target="${arch}-apple-darwin" ;;
esac

# --- resolve version --------------------------------------------------------
if [ -z "$VERSION" ]; then
	info "resolving latest release ..."
	VERSION="$(fetch "https://api.github.com/repos/${REPO}/releases/latest" |
		grep -m1 '"tag_name"' |
		sed -E 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/')"
	[ -n "$VERSION" ] || err "could not determine the latest version; pass --version vX.Y.Z"
fi
case "$VERSION" in v*) ;; *) VERSION="v${VERSION}" ;; esac
want="${VERSION#v}"

# --- idempotency / up-to-date check ----------------------------------------
dest="${OPP_INSTALL_DIR}/${BIN}"
if [ "$FORCE" -ne 1 ] && [ -x "$dest" ]; then
	current="$("$dest" --version 2>/dev/null | awk '{print $2}' || true)"
	if [ "$current" = "$want" ]; then
		info "opp ${want} is already installed at ${dest} — up to date."
		exit 0
	fi
	[ -n "$current" ] && info "upgrading opp ${current} -> ${want}"
fi

# --- download + verify + extract -------------------------------------------
asset="${BIN}-${VERSION}-${target}.tar.gz"
base="https://github.com/${REPO}/releases/download/${VERSION}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

info "downloading ${asset} ..."
download "${base}/${asset}" "${tmp}/${asset}" ||
	err "download failed — is ${VERSION} released for ${target}?"

if download "${base}/${asset}.sha256" "${tmp}/${asset}.sha256" 2>/dev/null; then
	if have sha256sum; then
		(cd "$tmp" && sha256sum -c "${asset}.sha256" >/dev/null 2>&1) || err "checksum verification failed"
		info "checksum ok"
	elif have shasum; then
		(cd "$tmp" && shasum -a 256 -c "${asset}.sha256" >/dev/null 2>&1) || err "checksum verification failed"
		info "checksum ok"
	else
		info "no sha256 tool found; skipping checksum verification"
	fi
else
	info "checksum file unavailable; skipping verification"
fi

tar -xzf "${tmp}/${asset}" -C "$tmp" || err "failed to extract ${asset}"
binpath="$(find "$tmp" -type f -name "$BIN" | head -n1)"
[ -n "$binpath" ] || err "binary '${BIN}' not found inside the archive"

# --- install atomically -----------------------------------------------------
mkdir -p "$OPP_INSTALL_DIR" || err "cannot create ${OPP_INSTALL_DIR}"
chmod +x "$binpath"
staged="${dest}.tmp.$$"
cp "$binpath" "$staged" || err "cannot write to ${OPP_INSTALL_DIR} (try --dir)"
mv -f "$staged" "$dest"
info "installed opp ${want} -> ${dest}"

# --- PATH hint + smoke check -----------------------------------------------
case ":${PATH}:" in
*":${OPP_INSTALL_DIR}:"*) ;;
*)
	info "note: ${OPP_INSTALL_DIR} is not on your PATH. Add it with:"
	info "  export PATH=\"${OPP_INSTALL_DIR}:\$PATH\""
	;;
esac
if "$dest" --version >/dev/null 2>&1; then
	info "done. Run 'opp --help' to get started."
fi

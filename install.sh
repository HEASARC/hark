#!/bin/sh

# Installer script for hark
#
# This script will attempt to download and install the latest release
# of hark for your system.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/HEASARC/hark/main/install.sh | sh
#
# To install a specific version (e.g., v0.1.0):
#   curl -sSL https://raw.githubusercontent.com/HEASARC/hark/main/install.sh | sh -s v0.1.0
#
# Script options (passed after 'sh -s --'):
#   --version <version_tag> : Install a specific version (e.g., v0.1.0)
#   --dry-run               : Print commands instead of executing them
#   --install-dir <path>    : Specify a custom installation directory (default: /usr/local/bin)
#
# Example with options:
#   curl -sSL ... | sh -s -- --version v0.1.0 --install-dir ~/bin

# --- Configuration ---
GITHUB_OWNER="HEASARC" # <<< REPLACE THIS with your GitHub username or organization
GITHUB_REPO="hark"
PROJECT_NAME="hark"
DEFAULT_INSTALL_DIR="${HOME}/.local/bin/"

# --- Helper Functions ---
echo_err() {
  printf "%s\n" "$*" >&2
}

# Check for required tools
require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo_err "Error: Required tool '$1' is not installed. Please install it and try again."
    exit 1
  fi
}

# --- Argument Parsing ---
TARGET_VERSION_ARG=""
DRY_RUN=0
INSTALL_DIR="${DEFAULT_INSTALL_DIR}"
mkdir -p ${INSTALL_DIR}

# Parse arguments passed via "sh -s -- <args>"
# If only one arg is passed without "--", assume it's the version for backward compatibility
if [ "$#" -eq 1 ] && ! echo "$1" | grep -qE '^-'; then
    TARGET_VERSION_ARG="$1"
    shift
fi

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      TARGET_VERSION_ARG="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift 1
      ;;
    --install-dir)
      INSTALL_DIR="$2"
      shift 2
      ;;
    -*)
      echo_err "Unknown option: $1"
      echo_err "Usage: $0 [--version <tag>] [--dry-run] [--install-dir <path>]"
      exit 1
      ;;
    *)
      # If a bare argument is passed and version not set, assume it's the version
      if [ -z "$TARGET_VERSION_ARG" ] && echo "$1" | grep -qE '^v?[0-9]+\.[0-9]+\.[0-9]+'; then
        TARGET_VERSION_ARG="$1"
      else
        echo_err "Unexpected argument: $1"
      fi
      shift 1
      ;;
  esac
done


# --- Main Logic ---
main() {
  echo "Starting ${PROJECT_NAME} installer..."
  echo "---"

  require_tool "curl"
  require_tool "tar"

  # 1. Detect OS and Architecture
  OS_KERNEL=$(uname -s)
  OS_ARCH=$(uname -m)

  TARGET_OS=""
  TARGET_ARCH=""
  ARCHIVE_EXT=".tar.gz" # hark releases use tar.gz for Linux/macOS

  case "$OS_KERNEL" in
    Linux)
      TARGET_OS="linux"
      ;;
    Darwin)
      TARGET_OS="macos"
      ;;
    *)
      echo_err "Error: Unsupported operating system '$OS_KERNEL'."
      echo_err "This script currently supports Linux and macOS."
      exit 1
      ;;
  esac

  case "$OS_ARCH" in
    x86_64 | amd64)
      TARGET_ARCH="amd64"
      ;;
    aarch64 | arm64) # arm64 is common on macOS M1/M2, aarch64 on Linux ARM
      TARGET_ARCH="arm64"
      ;;
    *)
      echo_err "Error: Unsupported architecture '$OS_ARCH'."
      echo_err "This script currently supports x86_64/amd64 and aarch64/arm64."
      exit 1
      ;;
  esac

  echo "Detected System: OS=${TARGET_OS}, Arch=${TARGET_ARCH}"

  # 2. Determine Release Version
  RELEASE_TAG=""
  if [ -z "$TARGET_VERSION_ARG" ]; then
    echo "Fetching latest release version from GitHub..."
    LATEST_RELEASE_URL="https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest"
    # Attempt to parse tag_name using grep and sed to avoid jq dependency
    RELEASE_TAG=$(curl -sSL --fail "$LATEST_RELEASE_URL" | grep '"tag_name":' | sed -E 's/.*"tag_name":[[:space:]]*"([^"]+)".*/\1/' | head -n 1)

    if [ -z "$RELEASE_TAG" ]; then
      echo_err "Error: Could not automatically fetch the latest release tag."
      echo_err "Please check the repository '${GITHUB_OWNER}/${GITHUB_REPO}' or specify a version manually:"
      echo_err "  curl ... | sh -s -- --version <tag>"
      exit 1
    fi
    echo "Latest release tag: $RELEASE_TAG"
  else
    RELEASE_TAG="$TARGET_VERSION_ARG"
    echo "Using specified version: $RELEASE_TAG"
  fi

  # Remove 'v' prefix from tag for asset naming, if present (e.g., v0.1.0 -> 0.1.0)
  VERSION=${RELEASE_TAG#v}

  # 3. Construct Asset Filename and Download URL
  ASSET_BASENAME="${PROJECT_NAME}-${VERSION}-${TARGET_OS}-${TARGET_ARCH}"
  ASSET_NAME="${ASSET_BASENAME}${ARCHIVE_EXT}"
  DOWNLOAD_URL="https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/${ASSET_NAME}"

  echo "Asset to download: $ASSET_NAME"
  echo "Download URL: $DOWNLOAD_URL"

  # Create a temporary directory for download and extraction
  # Use mktemp for secure temporary directory creation
  TMP_DIR=$(mktemp -d 2>/dev/null || mktemp -d -t "${PROJECT_NAME}-install.XXXXXX")
  if [ -z "$TMP_DIR" ]; then
    echo_err "Error: Could not create temporary directory."
    exit 1
  fi
  # shellcheck disable=SC2064 # $TMP_DIR is generated by mktemp, should be safe
  trap 'echo "Cleaning up temporary directory ${TMP_DIR}..."; rm -rf "${TMP_DIR}"' EXIT # Cleanup on exit

  DOWNLOAD_PATH="${TMP_DIR}/${ASSET_NAME}"

  # 4. Download Asset
  echo "---"
  echo "Downloading ${ASSET_NAME} to ${TMP_DIR}..."
  if [ "$DRY_RUN" -eq 1 ]; then
    echo "DRY RUN: curl -SL --progress-bar --fail -o \"${DOWNLOAD_PATH}\" \"${DOWNLOAD_URL}\""
  else
    curl -SL --progress-bar --fail -o "${DOWNLOAD_PATH}" "${DOWNLOAD_URL}"
    CURL_EXIT_CODE=$?
    if [ ${CURL_EXIT_CODE} -ne 0 ]; then
      echo_err "Error: Download failed (curl exit code: ${CURL_EXIT_CODE})."
      echo_err "Attempted to download from: ${DOWNLOAD_URL}"
      echo_err "Please check if the release and asset exist for your system and the specified version."
      exit 1
    fi
  fi

  # 5. Extract Archive
  echo "Extracting ${ASSET_NAME}..."
  # The binary name inside the archive is expected to be PROJECT_NAME (e.g., "hark")
  EXTRACTED_BINARY_PATH="${TMP_DIR}/${PROJECT_NAME}"

  if [ "$DRY_RUN" -eq 1 ]; then
    echo "DRY RUN: tar -xzf \"${DOWNLOAD_PATH}\" -C \"${TMP_DIR}\""
    echo "DRY RUN: # (Assuming binary is named ${PROJECT_NAME} inside the archive)"
    echo "DRY RUN: chmod +x \"${EXTRACTED_BINARY_PATH}\""
  else
    tar -xzf "${DOWNLOAD_PATH}" -C "${TMP_DIR}"
    if [ $? -ne 0 ]; then
      echo_err "Error: Extraction failed. The archive might be corrupt or not in the expected format."
      exit 1
    fi

    if [ ! -f "$EXTRACTED_BINARY_PATH" ]; then
        echo_err "Error: Extracted binary '${PROJECT_NAME}' not found in the archive at '${TMP_DIR}'."
        echo "Contents of ${TMP_DIR}:"
        ls -l "${TMP_DIR}"
        exit 1
    fi
    chmod +x "$EXTRACTED_BINARY_PATH"
  fi

  # 6. Install (Move to a directory in PATH)
  echo "---"
  echo "Attempting to install ${PROJECT_NAME} to ${INSTALL_DIR}..."
  FINAL_INSTALL_PATH="${INSTALL_DIR}/${PROJECT_NAME}"

  if [ "$DRY_RUN" -eq 1 ]; then
    echo "DRY RUN: mkdir -p \"${INSTALL_DIR}\""
    echo "DRY RUN: mv \"${EXTRACTED_BINARY_PATH}\" \"${FINAL_INSTALL_PATH}\""
    echo "DRY RUN: # (If '${INSTALL_DIR}' requires root, you might need to run 'sudo mv ...' manually)"
  else
    # Ensure install directory exists
    if ! mkdir -p "${INSTALL_DIR}"; then
        echo_err "Warning: Could not create installation directory '${INSTALL_DIR}'. Attempting to move anyway."
    fi

    # Try to move without sudo first
    if mv "$EXTRACTED_BINARY_PATH" "$FINAL_INSTALL_PATH" 2>/dev/null; then
      echo "${PROJECT_NAME} moved to ${FINAL_INSTALL_PATH}"
    else
      echo "Installation to '${FINAL_INSTALL_PATH}' requires root privileges."
      echo "Attempting with sudo (will prompt for password if needed)..."
      if sudo mv "$EXTRACTED_BINARY_PATH" "$FINAL_INSTALL_PATH"; then
        echo "${PROJECT_NAME} installed to ${FINAL_INSTALL_PATH} using sudo."
      else
        echo_err "Error: Failed to move ${PROJECT_NAME} to ${FINAL_INSTALL_PATH}, even with sudo."
        echo_err "Please try installing manually:"
        echo_err "  sudo mkdir -p \"${INSTALL_DIR}\""
        echo_err "  sudo mv \"${TMP_DIR}/${PROJECT_NAME}\" \"${FINAL_INSTALL_PATH}\""
        exit 1
      fi
    fi
  fi

  echo "---"
  echo "${PROJECT_NAME} version ${VERSION} installation complete!"
  echo ""
  echo "You can now try running:"
  echo "  ${PROJECT_NAME}"
  echo ""
  echo "If the command is not found, ensure '${INSTALL_DIR}' is in your system's PATH."
  echo "To check your PATH, run: echo \$PATH"
  echo "To uninstall, simply delete the binary: rm ${FINAL_INSTALL_PATH}"
  echo "---"
}

# Run the main function, passing along any arguments
main "$@"

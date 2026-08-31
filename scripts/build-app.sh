#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUILD_CONFIGURATION="${BUILD_CONFIGURATION:-release}"
APP_PATH="${1:-${PROJECT_DIR}/build/MyOCR.app}"

cd "${PROJECT_DIR}"
swift build -c "${BUILD_CONFIGURATION}" --product MyOCR

EXECUTABLE_PATH="${PROJECT_DIR}/.build/${BUILD_CONFIGURATION}/MyOCR"
mkdir -p "${APP_PATH}/Contents/MacOS" "${APP_PATH}/Contents/Resources"
cp "${EXECUTABLE_PATH}" "${APP_PATH}/Contents/MacOS/MyOCR"
cp "${PROJECT_DIR}/Resources/Info.plist" "${APP_PATH}/Contents/Info.plist"

SIGNING_IDENTITY="${CODESIGN_IDENTITY:-}"
if [[ -z "${SIGNING_IDENTITY}" ]]; then
    SIGNING_IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null \
        | sed -n 's/^[[:space:]]*[0-9]*) [A-F0-9]* "\(.*\)"$/\1/p' \
        | head -n 1)"
fi

if [[ -n "${SIGNING_IDENTITY}" ]]; then
    /usr/bin/codesign --force --deep --sign "${SIGNING_IDENTITY}" "${APP_PATH}" >/dev/null
    echo "Signed with ${SIGNING_IDENTITY}"
else
    /usr/bin/codesign --force --deep --sign - "${APP_PATH}" >/dev/null
    echo "No Apple signing identity found; using ad-hoc signing"
fi
echo "Built ${APP_PATH}"

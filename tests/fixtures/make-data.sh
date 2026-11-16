#!/usr/bin/env bash
# Build the small made-up Echo data folder the existing cases and scenarios use: `tests/fixtures/make-data.sh <dir>`.
# A wrapper for `make_data.py <dir> --set small` (see tests/fixtures/README.md for the other sets).
#
# <dir>/home is the data folder (ECHOCTL_HOME), <dir>/config the settings (ECHOCTL_CONFIG_DIR); <dir>/env sets
# both for a shell (`. <dir>/env`). Everything in it is invented (acme.example, globex.example): never real calls,
# people or companies. Times are relative to now, so Today has meetings behind and ahead of the line "now".
# The folder is rebuilt from nothing each time: give a folder of its own (a run's scratch folder), never ~/echo.
set -euo pipefail
exec python3 "$(dirname "$0")/make_data.py" "${1:?usage: make-data.sh <dir>}" --set small

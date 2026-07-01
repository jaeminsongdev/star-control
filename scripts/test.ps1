$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot

Push-Location $Root
try {
  python scripts\ci\run_all.py
} finally {
  Pop-Location
}

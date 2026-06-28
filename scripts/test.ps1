$ErrorActionPreference = "Stop"
Get-ChildItem -Recurse -Filter *.json | ForEach-Object {
  Get-Content -Raw $_.FullName | ConvertFrom-Json | Out-Null
}
Write-Host "JSON files parsed successfully."

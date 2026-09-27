# Compatibility entry point; the shared checker requires Python 3 and Cargo.
# New callers can run: python tools/check_dep_directions.py
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
try {
    & python (Join-Path $PSScriptRoot "check_dep_directions.py")
    exit $LASTEXITCODE
} catch {
    [Console]::Error.WriteLine("check_dep_directions: FAIL - Python 3 is required. $($_.Exception.Message)")
    exit 1
}

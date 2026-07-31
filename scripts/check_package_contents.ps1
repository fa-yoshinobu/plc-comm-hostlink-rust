[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$packageFiles = @(
    & cargo package --manifest-path (Join-Path $repositoryRoot "Cargo.toml") --allow-dirty --list |
        ForEach-Object { $_.Replace("\", "/") }
)
if ($LASTEXITCODE -ne 0) {
    throw "cargo package --list failed."
}

$forbiddenPrefixes = @(
    ".github/",
    "internal_docs/",
    "scripts/",
    "test/",
    "tests/",
    "tools/"
)
$forbiddenNames = @("AGENTS.md", "TODO.md", "release_check.bat", "run_ci.bat")
$forbidden = @(
    foreach ($path in $packageFiles) {
        if ($path -in $forbiddenNames) {
            $path
            continue
        }
        foreach ($prefix in $forbiddenPrefixes) {
            if ($path.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                $path
                break
            }
        }
    }
)
if ($forbidden.Count -ne 0) {
    throw "Registry package contains repository-only files: $($forbidden -join ', ')"
}

$required = @("Cargo.toml", "LICENSE", "README.md", "src/lib.rs")
$missing = @($required | Where-Object { $_ -notin $packageFiles })
if ($missing.Count -ne 0) {
    throw "Registry package is missing required files: $($missing -join ', ')"
}

Write-Host "[OK] Registry package content passed: files=$($packageFiles.Count)"

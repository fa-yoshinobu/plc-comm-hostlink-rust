[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$workspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot ".."))
$runId = [Guid]::NewGuid().ToString("N")
$workRoot = [System.IO.Path]::GetFullPath((Join-Path $workspaceRoot "plc-hostlink-crate-check-$runId"))
$targetRoot = Join-Path $workRoot "cargo-target"
$extractRoot = Join-Path $workRoot "extracted"
$consumerRoot = Join-Path $workRoot "consumer"
$workspacePrefix = $workspaceRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $workRoot.StartsWith($workspacePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Package-check work directory is outside the workspace: $workRoot"
}

$previousCargoTargetDir = $env:CARGO_TARGET_DIR
$previousRustdocFlags = $env:RUSTDOCFLAGS

try {
    New-Item -ItemType Directory -Path $targetRoot, $extractRoot, (Join-Path $consumerRoot "src") -Force | Out-Null
    $env:CARGO_TARGET_DIR = $targetRoot

    & cargo package --manifest-path (Join-Path $repositoryRoot "Cargo.toml") --allow-dirty --no-verify
    if ($LASTEXITCODE -ne 0) {
        throw "cargo package failed."
    }

    $crateFiles = @(Get-ChildItem -LiteralPath (Join-Path $targetRoot "package") -Filter "*.crate" -File)
    if ($crateFiles.Count -ne 1) {
        throw "Expected exactly one generated .crate, found $($crateFiles.Count)."
    }

    & tar -xzf $crateFiles[0].FullName -C $extractRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Cannot extract generated crate '$($crateFiles[0].Name)'."
    }

    $packageRoots = @(Get-ChildItem -LiteralPath $extractRoot -Directory)
    if ($packageRoots.Count -ne 1) {
        throw "Expected one package root in the generated crate, found $($packageRoots.Count)."
    }
    $packageRoot = $packageRoots[0].FullName
    $packagePrefix = $packageRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    $packageFiles = @(
        Get-ChildItem -LiteralPath $packageRoot -Recurse -File |
            ForEach-Object {
                $_.FullName.Substring($packagePrefix.Length).Replace("\", "/")
            } |
            Sort-Object -Unique
    )

    $forbiddenPrefixes = @(
        ".github/",
        "docs/",
        "internal_docs/",
        "scripts/",
        "test/",
        "tests/",
        "tools/"
    )
    $allowedRepositoryFiles = @("tests/fixtures/kv_device_ranges.json")
    $forbiddenNames = @("AGENTS.md", "TODO.md", "release_check.bat", "run_ci.bat")
    $forbidden = @(
        foreach ($path in $packageFiles) {
            if ($path -in $allowedRepositoryFiles) {
                continue
            }
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

    $required = @(
        "Cargo.toml",
        "LICENSE",
        "README.md",
        "src/lib.rs",
        "tests/fixtures/kv_device_ranges.json"
    )
    $missing = @($required | Where-Object { $_ -notin $packageFiles })
    if ($missing.Count -ne 0) {
        throw "Registry package is missing required files: $($missing -join ', ')"
    }

    $packagedManifest = Join-Path $packageRoot "Cargo.toml"
    $declaredExamples = @(
        Select-String -LiteralPath $packagedManifest -Pattern '^\s*path\s*=\s*"(examples/[^\"]+)"\s*$' |
            ForEach-Object { $_.Matches[0].Groups[1].Value.Replace("\", "/") } |
            Sort-Object -Unique
    )
    if ($declaredExamples.Count -eq 0) {
        throw "Generated crate declares no package-manager examples."
    }
    $missingExamples = @($declaredExamples | Where-Object { $_ -notin $packageFiles })
    if ($missingExamples.Count -ne 0) {
        throw "Generated crate is missing declared examples: $($missingExamples -join ', ')"
    }

    & cargo check --manifest-path $packagedManifest --all-features --lib --bins --examples
    if ($LASTEXITCODE -ne 0) {
        throw "Generated-crate library/binary/example check failed."
    }

    & cargo test --manifest-path $packagedManifest --all-features --lib
    if ($LASTEXITCODE -ne 0) {
        throw "Generated-crate library unit tests failed."
    }

    $env:RUSTDOCFLAGS = "-D warnings"
    & cargo doc --manifest-path $packagedManifest --no-deps --all-features
    if ($LASTEXITCODE -ne 0) {
        throw "Generated-crate rustdoc check failed."
    }

    $dependencyPath = $packageRoot.Replace("\", "/")
    $consumerManifest = @"
[package]
name = "plc-hostlink-package-consumer"
version = "0.0.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
plc-comm-kv-hostlink = { path = "$dependencyPath" }
"@
    $consumerSource = @"
use plc_comm_kv_hostlink::{
    HostLinkCommentEncoding, HostLinkOutcomeUnknownReason, HostLinkValue, NamedReadResult,
};

fn main() {
    let mut values = NamedReadResult::new();
    values.insert("DM0:U".to_owned(), HostLinkValue::U16(0));
    let reasons = [
        HostLinkOutcomeUnknownReason::Timeout,
        HostLinkOutcomeUnknownReason::Closed,
        HostLinkOutcomeUnknownReason::Transport,
        HostLinkOutcomeUnknownReason::MalformedResponse,
    ];
    let comment_encodings = [
        HostLinkCommentEncoding::Utf8,
        HostLinkCommentEncoding::Cp932,
    ];
    assert_eq!(values.len(), 1);
    assert_eq!(reasons.len(), 4);
    assert_eq!(comment_encodings.len(), 2);
}
"@
    [System.IO.File]::WriteAllText(
        (Join-Path $consumerRoot "Cargo.toml"),
        $consumerManifest,
        [System.Text.UTF8Encoding]::new($false)
    )
    [System.IO.File]::WriteAllText(
        (Join-Path $consumerRoot "src/main.rs"),
        $consumerSource,
        [System.Text.UTF8Encoding]::new($false)
    )

    & cargo check --manifest-path (Join-Path $consumerRoot "Cargo.toml")
    if ($LASTEXITCODE -ne 0) {
        throw "Generated-crate consumer check failed."
    }

    Write-Host "[OK] Generated crate consumer contract passed: crate=$($crateFiles[0].Name) files=$($packageFiles.Count) examples=$($declaredExamples.Count)"
}
finally {
    $env:RUSTDOCFLAGS = $previousRustdocFlags
    if ($null -eq $previousCargoTargetDir) {
        Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }
    else {
        $env:CARGO_TARGET_DIR = $previousCargoTargetDir
    }
    if (Test-Path -LiteralPath $workRoot) {
        Remove-Item -LiteralPath $workRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

[CmdletBinding()]
param(
    [string]$Treeish = "HEAD",
    [switch]$UseWorktreeAttributes
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$workspaceRoot = [System.IO.Directory]::GetParent($repositoryRoot).FullName
$runId = [guid]::NewGuid().ToString("N")
$archivePath = Join-Path $workspaceRoot ("plc-source-archive-$runId.zip")
$extractPath = Join-Path $workspaceRoot ("plc-source-archive-$runId")

$forbiddenFileNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
@(
    ".gitattributes",
    ".gitignore",
    ".pre-commit-config.yaml",
    "AGENTS.md",
    "release_check.bat",
    "run_ci.bat",
    "run-local-node-red.bat",
    "TODO.md"
) | ForEach-Object { [void]$forbiddenFileNames.Add($_) }

$forbiddenPrefixes = @(
    ".codex",
    ".github",
    ".pio",
    ".tools",
    "build",
    "build_win",
    "docsrc/maintainer",
    "internal_docs",
    "local_folder",
    "release-artifacts",
    "scripts",
    "tools"
)

try {
    & git -C $repositoryRoot rev-parse --verify "$Treeish`^{tree}" *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Cannot resolve treeish '$Treeish'."
    }

    $archiveArguments = @("archive", "--format=zip", "--output=$archivePath")
    if ($UseWorktreeAttributes) {
        $archiveArguments += "--worktree-attributes"
    }
    $archiveArguments += $Treeish
    & git -C $repositoryRoot @archiveArguments
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $archivePath)) {
        throw "git archive failed for '$Treeish'."
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($archivePath)
    try {
        $archiveFiles = @(
            $archive.Entries |
                ForEach-Object { $_.FullName.Replace("\", "/") } |
                Where-Object { -not $_.EndsWith("/") } |
                Sort-Object -Unique
        )
    }
    finally {
        $archive.Dispose()
    }

    $forbidden = @(
        foreach ($path in $archiveFiles) {
            $fileName = [System.IO.Path]::GetFileName($path)
            $lowerPath = $path.ToLowerInvariant()
            $hasForbiddenPrefix = $false
            foreach ($prefix in $forbiddenPrefixes) {
                $lowerPrefix = $prefix.ToLowerInvariant()
                if ($lowerPath -eq $lowerPrefix -or $lowerPath.StartsWith("$lowerPrefix/")) {
                    $hasForbiddenPrefix = $true
                    break
                }
            }
            if ($forbiddenFileNames.Contains($fileName) -or $hasForbiddenPrefix) {
                $path
            }
        }
    )
    if ($forbidden.Count -ne 0) {
        throw "Source archive contains maintainer-only files: $($forbidden -join ', ')"
    }

    $requiredRootFiles = @("CHANGELOG.md", "LICENSE", "README.md")
    $missingRootFiles = @($requiredRootFiles | Where-Object { $_ -notin $archiveFiles })
    if ($missingRootFiles.Count -ne 0) {
        throw "Source archive is missing required root files: $($missingRootFiles -join ', ')"
    }

    $expectedSamples = @(
        & git -C $repositoryRoot ls-tree -r --name-only $Treeish -- examples samples |
            ForEach-Object { $_.Replace("\", "/") } |
            Sort-Object -Unique
    )
    if ($LASTEXITCODE -ne 0) {
        throw "Cannot enumerate samples for '$Treeish'."
    }
    if ($expectedSamples.Count -eq 0) {
        throw "No tracked files were found under examples/ or samples/."
    }

    $actualSamples = @(
        $archiveFiles |
            Where-Object { $_.StartsWith("examples/") -or $_.StartsWith("samples/") } |
            Sort-Object -Unique
    )
    $sampleDifference = @(Compare-Object -ReferenceObject $expectedSamples -DifferenceObject $actualSamples -CaseSensitive)
    if ($sampleDifference.Count -ne 0) {
        $differenceText = ($sampleDifference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }) -join "; "
        throw "Source archive sample set differs from the tracked sample set: $differenceText"
    }

    $expectedTests = @(
        & git -C $repositoryRoot ls-tree -r --name-only $Treeish -- test tests |
            ForEach-Object { $_.Replace("\", "/") } |
            Sort-Object -Unique
    )
    if ($LASTEXITCODE -ne 0) {
        throw "Cannot enumerate tests for '$Treeish'."
    }
    if ($expectedTests.Count -eq 0) {
        throw "No tracked files were found under test/ or tests/."
    }

    $actualTests = @(
        $archiveFiles |
            Where-Object { $_.StartsWith("test/") -or $_.StartsWith("tests/") } |
            Sort-Object -Unique
    )
    $testDifference = @(Compare-Object -ReferenceObject $expectedTests -DifferenceObject $actualTests -CaseSensitive)
    if ($testDifference.Count -ne 0) {
        $differenceText = ($testDifference | ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }) -join "; "
        throw "Source archive test set differs from the tracked test set: $differenceText"
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $extractPath
    Push-Location $extractPath
    try {
        & cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) {
            throw "cargo fmt failed from the extracted source archive."
        }
        & cargo check --all-targets --all-features
        if ($LASTEXITCODE -ne 0) {
            throw "cargo check failed from the extracted source archive."
        }
        & cargo clippy --all-targets --all-features -- -D warnings
        if ($LASTEXITCODE -ne 0) {
            throw "cargo clippy failed from the extracted source archive."
        }
        $previousRustdocFlags = $env:RUSTDOCFLAGS
        $env:RUSTDOCFLAGS = "-D warnings"
        try {
            & cargo doc --no-deps --all-features
            if ($LASTEXITCODE -ne 0) {
                throw "cargo doc failed from the extracted source archive."
            }
        }
        finally {
            $env:RUSTDOCFLAGS = $previousRustdocFlags
        }
        & cargo test --all-targets --all-features
        if ($LASTEXITCODE -ne 0) {
            throw "cargo test failed from the extracted source archive."
        }
    }
    finally {
        Pop-Location
    }

    Write-Host "[OK] Source archive contract passed: treeish=$Treeish files=$($archiveFiles.Count) samples=$($actualSamples.Count) tests=$($actualTests.Count)"
}
finally {
    Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $extractPath -Recurse -Force -ErrorAction SilentlyContinue
}

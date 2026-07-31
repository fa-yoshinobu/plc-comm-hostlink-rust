[CmdletBinding()]
param(
    [string]$Treeish = "HEAD",
    [switch]$UseWorktreeAttributes,
    [switch]$UseCurrentWorktree
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$workspaceRoot = [System.IO.Directory]::GetParent($repositoryRoot).FullName
$runId = [guid]::NewGuid().ToString("N")
$archivePath = Join-Path $workspaceRoot ("plc-source-archive-$runId.zip")
$extractPath = Join-Path $workspaceRoot ("plc-source-archive-$runId")
$temporaryIndexPath = Join-Path $workspaceRoot ("plc-source-archive-$runId.index")

$forbiddenFileNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
@(
    ".gitattributes",
    ".gitignore"
) | ForEach-Object { [void]$forbiddenFileNames.Add($_) }

$forbiddenPrefixes = @(
    ".codex",
    ".pio",
    ".tools",
    "build",
    "build_win",
    "local_folder",
    "release-artifacts"
)

try {
    $effectiveTreeish = $Treeish
    if ($UseCurrentWorktree) {
        $previousIndexFile = $env:GIT_INDEX_FILE
        try {
            $env:GIT_INDEX_FILE = $temporaryIndexPath
            & git -C $repositoryRoot read-tree HEAD
            if ($LASTEXITCODE -ne 0) {
                throw "Cannot initialize the temporary current-worktree index."
            }
            & git -C $repositoryRoot add -A -- .
            if ($LASTEXITCODE -ne 0) {
                throw "Cannot stage the complete current worktree in the temporary index."
            }
            $effectiveTreeish = (& git -C $repositoryRoot write-tree).Trim()
            if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($effectiveTreeish)) {
                throw "Cannot create the synthetic current-worktree tree."
            }
        }
        finally {
            if ($null -eq $previousIndexFile) {
                Remove-Item Env:GIT_INDEX_FILE -ErrorAction SilentlyContinue
            }
            else {
                $env:GIT_INDEX_FILE = $previousIndexFile
            }
        }
    }

    & git -C $repositoryRoot rev-parse --verify "$effectiveTreeish`^{tree}" *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Cannot resolve treeish '$effectiveTreeish'."
    }

    $archiveArguments = @("archive", "--format=zip", "--output=$archivePath")
    if ($UseWorktreeAttributes -or $UseCurrentWorktree) {
        $archiveArguments += "--worktree-attributes"
    }
    $archiveArguments += $effectiveTreeish
    & git -C $repositoryRoot @archiveArguments
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $archivePath)) {
        throw "git archive failed for '$effectiveTreeish'."
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
    $trackedFiles = @(& git -C $repositoryRoot ls-tree -r --name-only $effectiveTreeish |
        ForEach-Object { $_.Replace("\", "/") } |
        Sort-Object -Unique)
    if ($LASTEXITCODE -ne 0) { throw "Cannot enumerate tracked files for '$effectiveTreeish'." }

    $requiredTracked = @($trackedFiles | Where-Object {
        $_ -match '^(test|tests|\.github|docsrc/maintainer|internal_docs|scripts|tools)/' -or
        $_ -in @("AGENTS.md", "TODO.md", "release_check.bat", "run_ci.bat")
    })
    $missingTracked = @($requiredTracked | Where-Object { $_ -notin $archiveFiles })
    if ($missingTracked.Count -ne 0) {
        throw "Source archive omits tracked validation or maintainer material: $($missingTracked -join ', ')"
    }

    foreach ($guide in @("GETTING_STARTED.md", "USAGE_GUIDE.md", "PROFILES.md", "GOTCHAS.md", "API_REFERENCE.md")) {
        $guideCandidates = @("docsrc/user/$guide", "docs/$guide")
        if (@($guideCandidates | Where-Object { $_ -in $archiveFiles }).Count -eq 0) {
            throw "Source archive is missing standard user guide '$guide'."
        }
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
        throw "Source archive contains forbidden generated or release-output files: $($forbidden -join ', ')"
    }

    $requiredRootFiles = @("CHANGELOG.md", "LICENSE", "README.md")
    $missingRootFiles = @($requiredRootFiles | Where-Object { $_ -notin $archiveFiles })
    if ($missingRootFiles.Count -ne 0) {
        throw "Source archive is missing required root files: $($missingRootFiles -join ', ')"
    }

    $expectedSamples = @(
        & git -C $repositoryRoot ls-tree -r --name-only $effectiveTreeish -- examples samples |
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
        & git -C $repositoryRoot ls-tree -r --name-only $effectiveTreeish -- test tests |
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

    $sourceLabel = if ($UseCurrentWorktree) { "current-worktree" } else { $effectiveTreeish }
    Write-Host "[OK] Source archive contract passed: source=$sourceLabel files=$($archiveFiles.Count) samples=$($actualSamples.Count) tests=$($actualTests.Count)"
}
finally {
    Remove-Item -LiteralPath $archivePath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $extractPath -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $temporaryIndexPath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath "$temporaryIndexPath.lock" -Force -ErrorAction SilentlyContinue
}

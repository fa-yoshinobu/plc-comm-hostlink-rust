@echo off
setlocal

echo ===================================================
echo [RELEASE] Host Link Rust release check
echo ===================================================

echo [1/3] Checking registry version...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check_registry_duplicate.ps1 -Registry crates -Package plc-comm-hostlink-rust -VersionSource cargo -ManifestPath Cargo.toml
if %errorlevel% neq 0 (
    echo [ERROR] Release version check failed.
    exit /b %errorlevel%
)

echo [2/3] Running tests...
cargo test --all-features
if %errorlevel% neq 0 (
    echo [ERROR] Tests failed.
    exit /b %errorlevel%
)

echo [3/3] Packaging dry run...
cargo package
if %errorlevel% neq 0 (
    echo [ERROR] Package dry run failed.
    exit /b %errorlevel%
)

echo ===================================================
echo [SUCCESS] Release check passed.
echo ===================================================
endlocal

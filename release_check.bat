@echo off
setlocal

echo ===================================================
echo [RELEASE] Host Link Rust release check
echo ===================================================

echo [1/6] Checking registry version...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check_registry_duplicate.ps1 -Registry crates -Package plc-comm-kv-hostlink -VersionSource cargo -ManifestPath Cargo.toml
if %errorlevel% neq 0 (
    echo [ERROR] Release version check failed.
    exit /b %errorlevel%
)

echo [2/6] Checking canonical HostLink profile fixtures...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\update_hostlink_profile_jsons.ps1 -FailIfChanged
if %errorlevel% neq 0 (
    echo [ERROR] Canonical HostLink profile JSON check failed.
    exit /b %errorlevel%
)

echo [3/6] Checking GitHub source archive contents...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check_source_archive.ps1
if %errorlevel% neq 0 (
    echo [ERROR] Source archive content check failed.
    exit /b %errorlevel%
)

echo [4/6] Checking registry package contents...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check_package_contents.ps1
if %errorlevel% neq 0 (
    echo [ERROR] Registry package content check failed.
    exit /b %errorlevel%
)

echo [5/6] Running CI...
call run_ci.bat
if %errorlevel% neq 0 (
    echo [ERROR] CI failed.
    exit /b %errorlevel%
)

echo [6/6] Packaging dry run...
cargo package --allow-dirty
if %errorlevel% neq 0 (
    echo [ERROR] Package dry run failed.
    exit /b %errorlevel%
)

echo ===================================================
echo [SUCCESS] Release check passed.
echo ===================================================
endlocal

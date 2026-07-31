@echo off
setlocal

echo ===================================================
echo [CI] Host Link Rust local gate
echo ===================================================

echo [1/5] Checking formatting...
cargo fmt --all --check
if %errorlevel% neq 0 exit /b %errorlevel%

echo [2/5] Running clippy...
cargo clippy --all-targets --all-features -- -D warnings
if %errorlevel% neq 0 exit /b %errorlevel%

echo [3/5] Checking rustdoc...
set "RUSTDOCFLAGS=-D warnings"
cargo doc --no-deps --all-features
if %errorlevel% neq 0 exit /b %errorlevel%
set "RUSTDOCFLAGS="

echo [4/5] Running tests...
cargo test --all-targets --all-features
if %errorlevel% neq 0 exit /b %errorlevel%

echo [5/5] Validating generated crate and isolated consumer...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\check_package_contents.ps1
if %errorlevel% neq 0 exit /b %errorlevel%

echo ===================================================
echo [SUCCESS] CI passed.
echo ===================================================
endlocal

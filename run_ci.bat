@echo off
setlocal

echo ===================================================
echo [CI] Host Link Rust local gate
echo ===================================================

echo [1/3] Checking formatting...
cargo fmt --all --check
if %errorlevel% neq 0 exit /b %errorlevel%

echo [2/3] Running clippy...
cargo clippy --all-targets --all-features -- -D warnings
if %errorlevel% neq 0 exit /b %errorlevel%

echo [3/3] Running tests...
cargo test --all-targets --all-features
if %errorlevel% neq 0 exit /b %errorlevel%

echo ===================================================
echo [SUCCESS] CI passed.
echo ===================================================
endlocal

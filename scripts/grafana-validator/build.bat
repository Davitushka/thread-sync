@echo off
REM SIEM-Lite: Build Grafana Validation .exe для Windows
REM Требует Python 3.10+ и pip
REM
REM Результат: scripts\grafana-validator\dist\grafana-validator.exe

echo ============================================================
echo  SIEM-Lite Grafana Validator — PyInstaller Build
echo ============================================================
echo.

REM Перейти в корень проекта
cd /d "%~dp0..\.."

REM Установить зависимости для валидатора
echo [1/3] Installing Python dependencies...
pip install -r tests\grafana\requirements.txt >nul 2>&1
if errorlevel 1 (
    echo ERROR: pip install failed
    exit /b 1
)
echo   ✓ Dependencies installed

REM Установить PyInstaller
echo [2/3] Installing PyInstaller...
pip install pyinstaller >nul 2>&1
if errorlevel 1 (
    echo ERROR: PyInstaller install failed
    exit /b 1
)
echo   ✓ PyInstaller installed

REM Собрать .exe
echo [3/3] Building grafana-validator.exe...
pyinstaller --onefile ^
    --name grafana-validator ^
    --icon=NONE ^
    --add-data "tests\grafana;tests\grafana" ^
    tests\grafana\validate_grafana.py

if errorlevel 1 (
    echo ERROR: Build failed
    exit /b 1
)

echo.
echo ============================================================
echo  BUILD COMPLETE
echo ============================================================
echo  Output: scripts\grafana-validator\dist\grafana-validator.exe
echo.
echo  Usage:
echo    scripts\grafana-validator\dist\grafana-validator.exe ^
echo      --url http://localhost:3000 ^
echo      --user admin ^
echo      --password ClickHousePass123!
echo.
echo  Help:
echo    scripts\grafana-validator\dist\grafana-validator.exe --help
echo ============================================================

REM Скопировать .exe в директорию скриптов
if exist "dist\grafana-validator.exe" (
    copy /Y "dist\grafana-validator.exe" "scripts\grafana-validator\" >nul
    echo   ✓ Copied to scripts\grafana-validator\
)

REM Очистка временных файлов PyInstaller
rmdir /s /q build >nul 2>&1
rmdir /s /q dist >nul 2>&1
del /q grafana-validator.spec >nul 2>&1

echo.

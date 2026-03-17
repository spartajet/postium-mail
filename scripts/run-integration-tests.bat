@echo off
REM 集成测试运行脚本 (Windows)
REM
REM 用法:
REM   run-integration-tests.bat [test_name]
REM
REM 示例:
REM   run-integration-tests.bat                    # 运行所有测试
REM   run-integration-tests.bat greenmail_connection # 运行特定测试

setlocal enabledelayedexpansion

REM 颜色定义（Windows 10+）
for /F %%a in ('echo prompt $E ^| cmd') do set "ESC=%%a"
set "INFO=[INFO]"
set "ERROR=[ERROR]"
set "WARN=[WARN]"

REM 函数：打印信息
:info
echo %ESC%[92m%INFO%%ESC%[0m %~1
goto :eof

:error
echo %ESC%[91m%ERROR%%ESC%[0m %~1
goto :eof

:warn
echo %ESC%[93m%WARN%%ESC%[0m %~1
goto :eof

REM 检查 Docker 是否运行
:check_docker
docker info >nul 2>&1
if %errorlevel% neq 0 (
    call :error "Docker 未运行，请先启动 Docker"
    exit /b 1
)
call :info "Docker 运行正常"
goto :eof

REM 启动 GreenMail
:start_greenmail
call :info "启动 GreenMail..."

REM 检查是否已经在运行
docker ps | findstr "postmium-greenmail" >nul
if %errorlevel% equ 0 (
    call :warn "GreenMail 已经在运行"
    goto :eof
)

docker-compose -f docker-compose.test.yml up -d

call :info "等待 GreenMail 就绪..."
set /a count=0
:wait_loop
if %count% geq 30 (
    call :error "GreenMail 启动超时"
    exit /b 1
)
curl -s http://localhost:8080/health >nul 2>&1
if %errorlevel% equ 0 (
    call :info "GreenMail 已就绪"
    goto :eof
)
timeout /t 1 /nobreak >nul
set /a count+=1
goto wait_loop

REM 停止 GreenMail
:stop_greenmail
call :info "停止 GreenMail..."
docker-compose -f docker-compose.test.yml down
call :info "GreenMail 已停止"
goto :eof

REM 运行测试
:run_tests
set test_name=%~1

call :info "进入 src-tauri 目录..."
cd src-tauri

if "%test_name%"=="" (
    call :info "运行所有集成测试..."
    cargo test --test integration -- --ignored --nocapture
) else (
    call :info "运行测试: %test_name%"
    cargo test "test_%test_name%" -- --ignored --nocapture
)

cd ..
goto :eof

REM 帮助信息
:show_help
echo 用法: %~nx0 [test_name] [--no-stop]
echo.
echo 参数:
echo   test_name    - 要运行的测试名称（可选）
echo   --no-stop    - 测试后不停止 GreenMail（可选）
echo.
echo 示例:
echo   %~nx0                                    # 运行所有测试
echo   %~nx0 greenmail_connection               # 运行特定测试
echo   %~nx0 --no-stop                          # 运行测试后保持 GreenMail 运行
echo.
echo 可用的测试:
echo   - greenmail_connection
echo   - greenmail_condstore_support
echo   - greenmail_folder_sync
echo   - greenmail_first_sync
echo   - greenmail_incremental_sync
echo   - delta_sync_condstore_strategy
exit /b 0

REM 主函数
:main
set test_name=%~1
set no_stop=%~2

echo ======================================
echo   Postium Mail 集成测试
echo ======================================
echo.

if "%test_name%"=="-h" goto show_help
if "%test_name%"=="--help" goto show_help

call :check_docker
if %errorlevel% neq 0 exit /b 1

call :start_greenmail
if %errorlevel% neq 0 exit /b 1
echo.

call :run_tests "%test_name%"
echo.

if not "%no_stop%"=="--no-stop" (
    call :stop_greenmail
) else (
    call :info "保持 GreenMail 运行 (使用 --no-stop 参数)"
    call :info "手动停止: docker-compose -f docker-compose.test.yml down"
)

echo.
call :info "测试完成！"
goto :eof

REM 运行主函数
call :main %*

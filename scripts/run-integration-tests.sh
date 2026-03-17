#!/bin/bash
# 集成测试运行脚本
#
# 用法:
#   ./scripts/run-integration-tests.sh [test_name]
#
# 示例:
#   ./scripts/run-integration-tests.sh                    # 运行所有测试
#   ./scripts/run-integration-tests.sh greenmail_connection # 运行特定测试

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 函数：打印信息
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# 检查 Docker 是否运行
check_docker() {
    if ! docker info > /dev/null 2>&1; then
        error "Docker 未运行，请先启动 Docker"
        exit 1
    fi
    info "Docker 运行正常"
}

# 启动 GreenMail
start_greenmail() {
    info "启动 GreenMail..."

    # 检查是否已经在运行
    if docker ps | grep -q "postmium-greenmail"; then
        warn "GreenMail 已经在运行"
        return
    fi

    docker-compose -f docker-compose.test.yml up -d

    info "等待 GreenMail 就绪..."
    for i in {1..30}; do
        if curl -s http://localhost:8080/health > /dev/null 2>&1; then
            info "GreenMail 已就绪"
            return
        fi
        sleep 1
    done

    error "GreenMail 启动超时"
    exit 1
}

# 停止 GreenMail
stop_greenmail() {
    info "停止 GreenMail..."
    docker-compose -f docker-compose.test.yml down
    info "GreenMail 已停止"
}

# 运行测试
run_tests() {
    local test_name="$1"

    info "进入 src-tauri 目录..."
    cd src-tauri

    if [ -z "$test_name" ]; then
        info "运行所有集成测试..."
        cargo test --test integration -- --ignored --nocapture
    else
        info "运行测试: $test_name"
        cargo test "test_$test_name" -- --ignored --nocapture
    fi

    cd ..
}

# 主函数
main() {
    local test_name="$1"
    local skip_stop="$2"

    echo "======================================"
    echo "  Postium Mail 集成测试"
    echo "======================================"
    echo

    check_docker
    start_greenmail
    echo

    run_tests "$test_name"
    echo

    if [ "$skip_stop" != "--no-stop" ]; then
        stop_greenmail
    else
        info "保持 GreenMail 运行 (使用 --no-stop 参数)"
        info "手动停止: docker-compose -f docker-compose.test.yml down"
    fi

    echo
    info "测试完成！"
}

# 帮助信息
show_help() {
    echo "用法: $0 [test_name] [--no-stop]"
    echo
    echo "参数:"
    echo "  test_name    - 要运行的测试名称（可选）"
    echo "  --no-stop    - 测试后不停止 GreenMail（可选）"
    echo
    echo "示例:"
    echo "  $0                                    # 运行所有测试"
    echo "  $0 greenmail_connection               # 运行特定测试"
    echo "  $0 --no-stop                          # 运行测试后保持 GreenMail 运行"
    echo
    echo "可用的测试:"
    echo "  - greenmail_connection"
    echo "  - greenmail_condstore_support"
    echo "  - greenmail_folder_sync"
    echo "  - greenmail_first_sync"
    echo "  - greenmail_incremental_sync"
    echo "  - delta_sync_condstore_strategy"
    exit 0
}

# 解析参数
if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
    show_help
fi

# 运行主函数
main "$@"

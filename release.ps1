#Requires -Version 5.1

<#
.SYNOPSIS
    轻墨小说写作工具发布脚本 (Rust标准方式)

.DESCRIPTION
    使用GitHub CLI和标准的Rust发布流程创建GitHub releases

.PARAMETER Version
    版本号 (例如: 0.1.0)

.PARAMETER DryRun
    仅预览发布内容，不实际执行

.EXAMPLE
    .\release.ps1 -Version "0.1.1"

.EXAMPLE
    .\release.ps1 -Version "0.1.1" -DryRun
#>

param(
    [Parameter(Mandatory = $true)]
    [string]$Version,

    [switch]$DryRun
)

# 配置
$ProjectName = "text-tool"
$Owner = "suifengwudong"
$Repo = "text-tool"
$TagName = "v$Version"
$ReleaseDir = "release\$TagName"

# 颜色输出函数
function Write-ColorOutput {
    param([string]$Message, [string]$Color = "White")
    Write-Host $Message -ForegroundColor $Color
}

# 检查命令是否存在
function Test-Command {
    param([string]$Command)
    try {
        Get-Command $Command -ErrorAction Stop | Out-Null
        return $true
    }
    catch {
        return $false
    }
}

# 主函数
function Main {
    Write-ColorOutput "🚀 发布 $TagName (Rust标准方式)" "Cyan"

    # 检查依赖
    Write-ColorOutput "📋 检查依赖..." "Yellow"
    $missingDeps = @()

    if (-not (Test-Command "cargo")) { $missingDeps += "cargo (Rust)" }
    if (-not (Test-Command "git")) { $missingDeps += "git" }
    if (-not (Test-Command "gh")) { $missingDeps += "gh (GitHub CLI)" }

    if ($missingDeps.Count -gt 0) {
        throw "缺少依赖: $($missingDeps -join ', ')"
    }

    # 检查工作目录
    if (-not (Test-Path "Cargo.toml")) {
        throw "请在项目根目录运行此脚本"
    }

    # 验证版本格式
    if ($Version -notmatch '^\d+\.\d+\.\d+(-[\w\.\-]+)?$') {
        throw "版本号格式无效，应为 x.y.z 或 x.y.z-suffix"
    }

    # 检查Git状态
    $gitStatus = & git status --porcelain
    if ($gitStatus) {
        Write-ColorOutput "⚠️ 工作目录有未提交的更改:" "Yellow"
        Write-ColorOutput $gitStatus "Red"
        $continue = Read-Host "是否继续? (y/N)"
        if ($continue -ne 'y' -and $continue -ne 'Y') {
            exit 0
        }
    }

    # 检查tag是否已存在
    $existingTag = & git tag -l $TagName 2>$null
    if ($existingTag) {
        throw "Tag $TagName 已存在"
    }

    # 构建release版本
    Write-ColorOutput "🔨 构建release版本..." "Yellow"
    if (-not $DryRun) {
        & cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "构建失败"
        }
    }
    Write-ColorOutput "✅ 构建完成" "Green"

    # 定义发布包路径（用于dry-run预览）
    $zipName = "$ProjectName-$TagName-windows-x64.zip"
    $zipPath = "$ReleaseDir\$zipName"

    # 创建release目录和文件
    if (-not $DryRun) {
        if (-not (Test-Path $ReleaseDir)) {
            New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null
        }

        # 复制可执行文件
        $exePath = "target\release\text_tool.exe"
        if (Test-Path $exePath) {
            Copy-Item $exePath $ReleaseDir
        } else {
            throw "未找到可执行文件: $exePath"
        }

        # 复制README
        Copy-Item "README.md" "$ReleaseDir\" -Force

        # 创建压缩包
        Write-ColorOutput "📦 创建发布包..." "Yellow"
        Compress-Archive -Path "$ReleaseDir\*" -DestinationPath $zipPath -Force
        Write-ColorOutput "✅ 发布包创建完成: $zipPath" "Green"
    } else {
        Write-ColorOutput "📦 预览：将创建发布包 $zipName" "Cyan"
    }

    # 创建git tag
    Write-ColorOutput "🏷️ 创建git tag..." "Yellow"
    $tagMessage = @"
Release $TagName

轻墨小说写作工具 $TagName 发布

主要特性:
- VS Code风格UI界面，完全中文化
- 模块化面板：小说编辑、人设管理、大纲伏笔、LLM辅助
- 轻量化设计：安装包<5MB，启动<0.5秒，内存<50MB
- 本地优先架构，MD/JSON纯文本存储
- 自动保存、文件侧边栏隐藏、文本模板系统

技术栈:
- Rust + egui (轻量化GUI框架)
- 本地LLM支持
- 跨平台兼容性
"@

    if (-not $DryRun) {
        $tagMessage | & git tag -a $TagName -F -
        & git push origin $TagName
        Write-ColorOutput "✅ Git tag创建并推送完成" "Green"
    }

    # 使用GitHub CLI创建release
    Write-ColorOutput "🚀 使用GitHub CLI创建release..." "Yellow"

    # 读取release说明
    $releaseNotes = Get-Content "README.md" -Raw

    # 构建gh release命令
    $ghArgs = @(
        "release", "create", $TagName,
        "--title", "$ProjectName $TagName",
        "--notes", $releaseNotes,
        "--latest"
    )

    # 添加预发布标志
    if ($Version.Contains("alpha") -or $Version.Contains("beta") -or $Version.Contains("rc")) {
        $ghArgs += "--prerelease"
    }

    # 添加文件
    if (Test-Path $zipPath) {
        $ghArgs += $zipPath
    }

    try {
        # 执行GitHub CLI命令
        & gh @ghArgs
        if ($LASTEXITCODE -ne 0) {
            throw "GitHub release创建失败"
        }
        Write-ColorOutput "✅ GitHub release创建成功" "Green"
    } catch {
        Write-ColorOutput "⚠️ GitHub CLI认证失败，但发布文件已准备就绪" "Yellow"
        Write-ColorOutput "📋 请手动在GitHub上创建release:" "Cyan"
        Write-ColorOutput "   1. 访问: https://github.com/$Owner/$Repo/releases/new" "White"
        Write-ColorOutput "   2. Tag: $TagName" "White"
        Write-ColorOutput "   3. Title: $ProjectName $TagName" "White"
        Write-ColorOutput "   4. 上传文件: $zipPath" "White"
        Write-ColorOutput "   5. 发布说明请复制README.md内容" "White"
        Write-ColorOutput "" "White"
        Write-ColorOutput "🔍 预览命令 (用于手动执行):" "Cyan"
        Write-ColorOutput "gh $($ghArgs -join ' ')" "Gray"
    }

    # 显示发布信息
    Write-ColorOutput "`n🎉 发布完成！" "Green"
    Write-ColorOutput "📁 Release文件位置: $ReleaseDir" "Cyan"
    Write-ColorOutput "🏷️ Tag: $TagName" "Cyan"
    Write-ColorOutput "🔗 GitHub Release: https://github.com/$Owner/$Repo/releases/tag/$TagName" "Cyan"

    if ($DryRun) {
        Write-ColorOutput "`n💡 这是预览模式。要实际发布，请移除 -DryRun 参数" "Yellow"
    }
}

# 执行主函数
try {
    Main
} catch {
    Write-ColorOutput "❌ 发布失败: $($_.Exception.Message)" "Red"
    exit 1
}
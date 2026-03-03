# 发布指南 (Rust标准方式)

## Rust项目的正确发布方式

与Node.js使用npm发布到npm registry类似，Rust项目有以下发布方式：

### 1. 库项目 → crates.io
```bash
cargo login
cargo publish
```

### 2. 二进制应用 → GitHub Releases
```bash
# 使用GitHub CLI (推荐)
gh release create v1.0.0 --generate-notes

# 或使用我们的自动化脚本
.\release.ps1 -Version "1.0.0"
```

### 3. 自动化 → GitHub Actions
在 `.github/workflows/release.yml` 中配置自动发布

## 目录结构

```
release/
├── v0.1.0/
│   ├── text_tool.exe          # 可执行文件
│   ├── README.md              # 发布说明
│   └── text-tool-v0.1.0-windows-x64.zip  # 发布包
├── v0.1.1/
│   └── ...
└── ...
```

## 发布流程 (推荐方式)

### 使用GitHub CLI (最简单)

1. **安装GitHub CLI**
   ```bash
   # Windows (winget)
   winget install --id GitHub.cli

   # 或下载安装包
   # https://github.com/cli/cli/releases
   ```

2. **认证**
   ```bash
   gh auth login
   ```

3. **创建release**
   ```bash
   # 自动生成发布说明
   gh release create v0.1.1 --generate-notes

   # 或指定文件
   gh release create v0.1.1 ./release/v0.1.1/text-tool-v0.1.1-windows-x64.zip
   ```

### 使用自动化脚本

```powershell
# 预览发布内容
.\release.ps1 -Version "0.1.1" -DryRun

# 实际发布
.\release.ps1 -Version "0.1.1"
```

脚本会自动：
- ✅ 构建release版本 (`cargo build --release`)
- ✅ 创建release目录结构
- ✅ 生成压缩包
- ✅ 创建git tag并推送
- ✅ 使用GitHub CLI创建release并上传文件

## 版本号规范

遵循 [Semantic Versioning](https://semver.org/)：

- **MAJOR.MINOR.PATCH** (例如: 1.2.3)
- **预发布版本**: 1.0.0-alpha.1, 1.0.0-beta.2, 1.0.0-rc.1

## 发布检查清单

- [ ] 代码通过所有测试 (`cargo test`)
- [ ] 代码编译成功 (`cargo build --release`)
- [ ] 更新了README中的版本信息
- [ ] 更新了CHANGELOG（如果有）
- [ ] 提交了所有更改到main分支
- [ ] 准备好了GitHub CLI认证 (`gh auth status`)

## GitHub Actions自动化发布

创建 `.github/workflows/release.yml`：

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: windows-latest

    steps:
    - name: Checkout code
      uses: actions/checkout@v4

    - name: Setup Rust
      uses: dtolnay/rust-toolchain@stable

    - name: Build release
      run: cargo build --release

    - name: Create release archive
      run: |
        mkdir release
        cp target/release/text_tool.exe release/
        cp README.md release/
        Compress-Archive -Path "release/*" -DestinationPath "text-tool-${{ github.ref_name }}-windows-x64.zip"

    - name: Create GitHub release
      uses: softprops/action-gh-release@v1
      with:
        files: text-tool-${{ github.ref_name }}-windows-x64.zip
        generate_release_notes: true
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

推送tag时会自动创建release！

## 故障排除

### GitHub CLI认证问题
```bash
# 检查认证状态
gh auth status

# 重新认证
gh auth login
```

### 构建失败
```bash
# 清理并重新构建
cargo clean
cargo build --release
```

### 权限问题
确保GitHub CLI有发布权限，或使用Personal Access Token。

## 最佳实践

1. **始终测试release构建**：`cargo build --release`
2. **使用有意义的tag消息**
3. **保持发布说明更新**
4. **定期清理旧的release文件**
5. **使用GitHub Actions自动化重复工作**

这样就符合Rust生态的标准发布方式了！🚀
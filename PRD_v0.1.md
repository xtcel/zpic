> **zpic 是一个 Rust 编写的独立图床 CLI / SDK / Zed 插件 / MCP 工具链，兼容 PicGo 配置，但不依赖 PicGo 运行时。**

核心思路是：**先做 zpic-cli，把上传、配置、格式化、历史、迁移能力做扎实；再做 Zed 插件，把 Zed 当成入口；最后做 MCP，把 zpic 变成 AI Agent 可调用的图片工具。**

---

# 1. 项目总体定位

## 1.1 项目名称

建议：

```text
zpic
```

子项目：

```text
zpic-cli      # 命令行工具
zpic-core     # 核心库
zpic-zed      # Zed 插件
zpic-mcp      # MCP Server
zpic-config   # PicGo 配置兼容层，可并入 core
```

产品一句话：

> zpic 是一个 Rust 编写的跨平台图床工具，兼容 PicGo 配置文件，支持 S3/R2、GitHub、阿里云 OSS、腾讯云 COS、七牛云、SM.MS、本地目录等图床，并可集成到 Zed、MCP 和 AI 编程工作流中。

---

# 2. 为什么要兼容 PicGo 配置？

PicGo 已经积累了大量用户。PicGo-Core 默认配置文件路径是 `~/.picgo/config.json`，命令行也支持通过 `-c <path>` 指定配置文件；PicGo GUI 的配置路径则因系统不同而不同，例如 macOS 通常在 `~/Library/Application Support/picgo/data.json`，Linux 通常在 `~/.config/picgo/data.json`。([docs.picgo.app][1])

Typora 的图片上传文档也提到：PicGo.app 和 PicGo-Core 使用不同配置文件，但可以复制 PicGo.app 配置中的 `picBed` 对象到 PicGo-Core 配置中。([support.typora.io][2])

所以你兼容 PicGo 配置的价值非常明确：

1. 用户不用重新配置图床。
2. 能从 PicGo / vs-picgo 平滑迁移。
3. 先支持主流 `picBed` 配置结构，就能快速覆盖大量用户。
4. zpic 可以变成“Rust 原生 PicGo-Core 替代品”。

---

# 3. 总体架构设计

建议用 **Rust Workspace**。

```text
zpic/
├── Cargo.toml
├── crates/
│   ├── zpic-core/              # 核心上传 SDK
│   ├── zpic-config/            # 配置解析，兼容 PicGo
│   ├── zpic-cli/               # CLI 命令
│   ├── zpic-uploaders/         # 上传器集合，也可以拆多个 crate
│   ├── zpic-image/             # 图片处理
│   ├── zpic-history/           # 上传历史
│   ├── zpic-mcp/               # MCP Server
│   └── zpic-zed-helper/        # 给 Zed 插件调用的本地 helper，可选
├── extensions/
│   └── zpic-zed/               # Zed extension，Rust/Wasm
├── docs/
│   ├── config.md
│   ├── picgo-compatible.md
│   ├── zed.md
│   └── mcp.md
└── examples/
    ├── picgo-config/
    ├── r2/
    ├── github/
    └── local/
```

核心设计原则：

```text
zpic-cli 只是入口
zpic-core 才是核心
zpic-zed 调用 zpic-cli 或 zpic-core 能力
zpic-mcp 复用 zpic-core
```

不要把上传逻辑写死在 CLI 里，否则后面做 Zed 插件和 MCP 会重复实现。

---

# 4. 三阶段路线

## 阶段一：zpic-cli

目标：

```text
做一个独立可运行的 Rust 图床 CLI。
```

核心能力：

```bash
zpic upload ./demo.png
zpic upload ./demo.png --copy
zpic upload ./demo.png --format markdown
zpic upload ./demo.png --uploader github
zpic upload ./demo.png --config ~/.picgo/config.json
zpic upload --clipboard
zpic migrate README.md
zpic doctor
zpic config import-picgo
```

## 阶段二：Zed 插件 zpic

目标：

```text
让 Zed 用户可以在 Zed 中调用 zpic 上传图片。
```

Zed 当前扩展能力不是 VS Code 那种完整 Extension Host。Zed 官方资料显示，目前扩展主要支持语言、调试器、MCP servers、主题、图标主题等类型，未来才计划扩大 UI 自定义能力。([Zed][3])

Zed Rust Extension API 提供了 `process`、`http_client`、`settings` 等模块，可以用于调用本地 CLI 或读取配置。([文档.rs][4])

因此 Zed 插件第一版建议是：

```text
Zed Slash Command / Task / MCP 入口
        ↓
调用本地 zpic-cli
        ↓
返回 Markdown / URL
```

不要第一版就追求：

```text
粘贴图片自动上传
拖拽图片自动上传
右键菜单上传
图片管理面板
```

这些目前在 Zed 里不适合作为 MVP。

## 阶段三：MCP / AI 功能

目标：

```text
让 AI Agent 可以调用 zpic 上传图片、迁移 Markdown 图片、生成图片链接、整理图床历史。
```

官方 Rust MCP SDK 已经存在，核心 crate 是 `rmcp`，基于 tokio async runtime。([GitHub][5])

MCP 可以提供工具：

```text
upload_image
upload_clipboard_image
migrate_markdown_images
list_upload_history
delete_uploaded_image
get_image_info
```

不过 MCP 工具涉及本地文件和远程上传，要特别注意安全边界。近年的 MCP 生态安全研究也指出，MCP Server 存在工具投毒、命令执行、维护性和权限边界等风险，设计时要做路径限制、命令白名单、确认机制和日志审计。([arXiv][6])

---

# 5. zpic-cli 功能设计

## 5.1 命令结构

建议使用 `clap`。

```bash
zpic upload <files...>
zpic upload --clipboard
zpic upload-url <url>
zpic migrate <markdown-file-or-dir>
zpic config init
zpic config show
zpic config import-picgo
zpic config convert-picgo
zpic history list
zpic history open
zpic history delete <id>
zpic doctor
zpic server
zpic mcp
```

---

## 5.2 上传图片

```bash
zpic upload ./cover.png
```

输出：

```md
![cover](https://cdn.example.com/images/2026/06/04/a8f32d19.png)
```

多文件：

```bash
zpic upload ./a.png ./b.jpg ./c.webp
```

输出：

```md
![a](https://cdn.example.com/images/2026/06/04/a.png)
![b](https://cdn.example.com/images/2026/06/04/b.jpg)
![c](https://cdn.example.com/images/2026/06/04/c.webp)
```

复制到剪贴板：

```bash
zpic upload ./cover.png --copy
```

JSON 输出：

```bash
zpic upload ./cover.png --json
```

```json
{
  "success": true,
  "items": [
    {
      "source": "/Users/yong/docs/cover.png",
      "url": "https://cdn.example.com/images/2026/06/04/a8f32d19.png",
      "key": "images/2026/06/04/a8f32d19.png",
      "markdown": "![cover](https://cdn.example.com/images/2026/06/04/a8f32d19.png)",
      "mime": "image/png",
      "size": 238912,
      "width": 1200,
      "height": 800,
      "uploader": "r2"
    }
  ]
}
```

---

## 5.3 剪贴板上传

```bash
zpic upload --clipboard --copy
```

实现难度：

| 系统            | 方案                        |
| ------------- | ------------------------- |
| macOS         | `arboard` 或系统 API         |
| Windows       | `arboard` / clipboard-win |
| Linux X11     | xclip / xcb               |
| Linux Wayland | wl-clipboard              |

建议第一版：

```text
优先支持 macOS + Windows
Linux 第二阶段完善
```

因为你的主要开发环境是 macOS，先保证自己能用。

---

## 5.4 Markdown 图片迁移

这个功能很重要，甚至可以作为 zpic 的差异化卖点。

```bash
zpic migrate README.md
```

扫描：

```md
![logo](./assets/logo.png)
![cover](../images/cover.jpg)
```

上传后替换为：

```md
![logo](https://cdn.example.com/images/2026/06/04/logo.png)
![cover](https://cdn.example.com/images/2026/06/04/cover.jpg)
```

支持 dry-run：

```bash
zpic migrate README.md --dry-run
```

支持目录：

```bash
zpic migrate ./docs --recursive
```

支持只处理本地图片：

```bash
zpic migrate ./docs --local-only
```

支持忽略远程图片：

```bash
zpic migrate ./docs --ignore-remote
```

支持输出报告：

```bash
zpic migrate ./docs --report migration-report.json
```

---

# 6. PicGo 配置兼容方案

## 6.1 配置来源优先级

建议 zpic 的配置读取优先级如下：

```text
1. 命令行 --config 指定
2. 环境变量 ZPIC_CONFIG
3. 当前项目 .zpic/config.toml
4. 用户配置 ~/.config/zpic/config.toml
5. PicGo-Core 配置 ~/.picgo/config.json
6. PicGo GUI 配置 data.json
```

其中 PicGo 配置只作为兼容来源，不建议直接修改原文件。

---

## 6.2 zpic 原生配置格式

建议 zpic 原生配置用 TOML。

路径：

```bash
~/.config/zpic/config.toml
```

示例：

```toml
default_uploader = "r2"
default_format = "markdown"
copy_after_upload = true
history_enabled = true

[rename]
strategy = "date-hash"
path = "images/{yyyy}/{mm}/{dd}/{hash8}.{ext}"
keep_original_name = false

[format]
markdown = "![{alt}]({url})"
html = "<img src=\"{url}\" alt=\"{alt}\" />"
url = "{url}"

[uploaders.r2]
type = "s3"
endpoint = "https://xxxx.r2.cloudflarestorage.com"
region = "auto"
bucket = "blog-images"
access_key_id = "$R2_ACCESS_KEY_ID"
secret_access_key = "$R2_SECRET_ACCESS_KEY"
public_base_url = "https://cdn.example.com"
path_prefix = "images/{yyyy}/{mm}/{dd}"

[uploaders.github]
type = "github"
repo = "username/picbed"
branch = "main"
token = "$GITHUB_TOKEN"
path_prefix = "images/{yyyy}/{mm}/{dd}"
public_base_url = "https://cdn.jsdelivr.net/gh/username/picbed@main"

[uploaders.local]
type = "local"
target_dir = "./public/images"
public_base_url = "/images"
path_prefix = "{yyyy}/{mm}/{dd}"
```

---

## 6.3 PicGo 配置导入

PicGo 常见配置核心是：

```json
{
  "picBed": {
    "current": "github",
    "uploader": "github",
    "github": {
      "repo": "username/repo",
      "branch": "main",
      "token": "xxx",
      "path": "img/",
      "customUrl": "https://cdn.jsdelivr.net/gh/username/repo"
    }
  },
  "picgoPlugins": {}
}
```

zpic 可以支持两种方式。

### 方式 A：直接读取 PicGo 配置

```bash
zpic upload ./demo.png --picgo-config ~/.picgo/config.json
```

或者：

```bash
zpic upload ./demo.png --config ~/.picgo/config.json
```

内部识别到 JSON 中有 `picBed`，就走 PicGo 兼容解析。

### 方式 B：转换为 zpic 配置

```bash
zpic config import-picgo
```

输出：

```bash
Imported PicGo config from ~/.picgo/config.json
Generated zpic config at ~/.config/zpic/config.toml
```

建议默认使用 **方式 B**。

原因：

1. zpic 不应该直接修改 PicGo 配置。
2. PicGo 配置字段不够统一。
3. zpic 后续有更多能力，比如 history、rename、migrate、MCP 权限，不适合塞进 PicGo 配置。

---

## 6.4 PicGo 兼容范围

第一版不要承诺兼容所有 PicGo 插件，只兼容内置常见图床配置。

### v0.1 兼容

```text
github
smms
qiniu
upyun
aliyun
tcyun / tencent-cloud-cos
imgur
```

### v0.2 兼容

```text
s3
webdav
local
custom
```

### 暂不兼容

```text
第三方 PicGo 插件的自定义 uploader
```

原因：PicGo 插件是 Node 生态，Rust 无法直接运行 Node 插件。你可以做“配置兼容”，但不能做“插件运行时兼容”。

这点要在文档中说清楚：

> zpic 兼容 PicGo 配置文件中的主流图床配置，但不加载 PicGo Node 插件。

---

# 7. 核心 Rust 模块设计

## 7.1 zpic-core

核心接口：

```rust
#[async_trait::async_trait]
pub trait Uploader: Send + Sync {
    fn name(&self) -> &'static str;

    async fn upload(
        &self,
        ctx: UploadContext,
        input: UploadInput,
    ) -> Result<UploadOutput, ZpicError>;
}
```

数据结构：

```rust
pub struct UploadInput {
    pub source_path: PathBuf,
    pub file_name: String,
    pub mime: String,
    pub bytes: bytes::Bytes,
    pub size: u64,
    pub alt: Option<String>,
}

pub struct UploadContext {
    pub target_key: String,
    pub config: Arc<ZpicConfig>,
    pub dry_run: bool,
}

pub struct UploadOutput {
    pub url: String,
    pub key: String,
    pub markdown: String,
    pub size: u64,
    pub mime: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub uploader: String,
}
```

---

## 7.2 zpic-config

职责：

```text
读取 zpic 原生配置
读取 PicGo 配置
转换 PicGo 配置
合并命令行参数
解析环境变量
脱敏输出配置
校验配置
```

建议结构：

```rust
pub enum ConfigSource {
    ZpicToml(PathBuf),
    PicgoCoreJson(PathBuf),
    PicgoGuiJson(PathBuf),
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load(path: Option<PathBuf>) -> Result<ZpicConfig, ZpicError>;
    pub fn import_picgo(path: PathBuf) -> Result<ZpicConfig, ZpicError>;
}
```

PicGo 配置解析：

```rust
#[derive(Debug, Deserialize)]
pub struct PicGoConfig {
    #[serde(rename = "picBed")]
    pub pic_bed: Option<PicBed>,
    #[serde(rename = "picgoPlugins")]
    pub picgo_plugins: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PicBed {
    pub current: Option<String>,
    pub uploader: Option<String>,

    #[serde(flatten)]
    pub uploaders: HashMap<String, serde_json::Value>,
}
```

然后根据 `current` / `uploader` 找到当前图床配置。

---

## 7.3 zpic-uploaders

建议第一批内置上传器：

| 上传器           | 优先级 | 原因                |
| ------------- | --: | ----------------- |
| local         |  P0 | 最容易，适合静态博客        |
| github        |  P0 | 免费用户多             |
| s3-compatible |  P0 | 覆盖 R2、S3、MinIO、B2 |
| smms          |  P1 | 简单 API            |
| aliyun-oss    |  P1 | 国内常用              |
| tencent-cos   |  P1 | 国内常用              |
| qiniu         |  P2 | PicGo 用户常见        |
| upyun         |  P2 | PicGo 用户常见        |
| imgur         |  P2 | 海外常见              |

### 最重要的是 S3-Compatible

S3-Compatible 一套可以覆盖：

```text
AWS S3
Cloudflare R2
MinIO
Backblaze B2
Wasabi
```

Rust 可以用：

```toml
aws-sdk-s3 = "..."
aws-config = "..."
```

或者用更轻量的 S3 crate。为了稳定，建议先用官方 AWS SDK，但要注意二进制体积。

---

## 7.4 zpic-image

职责：

```text
检测 MIME
读取尺寸
计算 hash
图片压缩
格式转换
EXIF 清理
```

第一版只做：

```text
MIME 检测
尺寸读取
hash
文件扩展名判断
```

第二版再做：

```text
压缩
webp
avif
resize
```

可选 crate：

```toml
image = "..."
infer = "..."
sha2 = "..."
blake3 = "..."
```

hash 建议用 `blake3`，速度快。

---

## 7.5 zpic-history

上传历史建议用 SQLite。

路径：

```bash
~/.local/share/zpic/history.db
```

或者：

```bash
~/Library/Application Support/zpic/history.db
```

使用 `directories` crate 处理跨平台路径。

表结构：

```sql
CREATE TABLE uploads (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    source_path TEXT,
    source_hash TEXT,
    uploader TEXT NOT NULL,
    key TEXT NOT NULL,
    url TEXT NOT NULL,
    markdown TEXT NOT NULL,
    mime TEXT,
    size INTEGER,
    width INTEGER,
    height INTEGER,
    status TEXT NOT NULL
);
```

历史命令：

```bash
zpic history list
zpic history list --uploader r2
zpic history search cover
zpic history copy <id>
zpic history delete <id>
```

---

# 8. 上传器详细设计

## 8.1 Local Uploader

配置：

```toml
[uploaders.local]
type = "local"
target_dir = "./public/images"
public_base_url = "/images"
path_prefix = "{yyyy}/{mm}/{dd}"
```

流程：

```text
读取文件
生成 target_key
复制到 target_dir/target_key
返回 public_base_url/target_key
```

适合：

```text
Next.js
Astro
VitePress
Docusaurus
Hugo
Hexo
```

---

## 8.2 GitHub Uploader

配置：

```toml
[uploaders.github]
type = "github"
repo = "username/picbed"
branch = "main"
token = "$GITHUB_TOKEN"
path_prefix = "images/{yyyy}/{mm}/{dd}"
public_base_url = "https://cdn.jsdelivr.net/gh/username/picbed@main"
```

流程：

```text
读取文件
base64
PUT /repos/{owner}/{repo}/contents/{path}
生成 CDN URL
```

PicGo 的 VS Code 插件也支持选择当前图床、输入图床信息、自定义上传文件名等能力，你可以把这些能力做成 CLI 参数和配置项，而不是做成 GUI。([GitHub][7])

---

## 8.3 S3 / R2 Uploader

配置：

```toml
[uploaders.r2]
type = "s3"
endpoint = "https://xxxx.r2.cloudflarestorage.com"
region = "auto"
bucket = "blog-images"
access_key_id = "$R2_ACCESS_KEY_ID"
secret_access_key = "$R2_SECRET_ACCESS_KEY"
public_base_url = "https://cdn.example.com"
path_prefix = "images/{yyyy}/{mm}/{dd}"
```

流程：

```text
读取文件
生成 key
PUT Object
设置 content-type
返回 public_base_url/key
```

建议加：

```toml
acl = "private"
cache_control = "public, max-age=31536000, immutable"
```

R2 通常不需要传统 ACL，public URL 由自定义域名控制。

---

## 8.4 阿里云 OSS / 腾讯云 COS

建议第二阶段做。

这两个的 SDK 体积、认证和 region 配置相对更复杂。第一版可以先用 S3-Compatible + GitHub + Local，把核心跑通。

---

# 9. 文件命名与路径模板

这是图床工具的长期体验核心。

## 9.1 默认路径模板

建议默认：

```text
images/{yyyy}/{mm}/{dd}/{hash8}.{ext}
```

例如：

```text
images/2026/06/04/a8f32d19.png
```

## 9.2 支持变量

```text
{yyyy}
{yy}
{mm}
{dd}
{hh}
{min}
{ss}
{timestamp}
{unix}
{name}
{slug}
{hash}
{hash8}
{uuid}
{ext}
{random}
```

## 9.3 命令行覆盖

```bash
zpic upload ./cover.png --name blog-cover
```

生成：

```text
images/2026/06/04/blog-cover.png
```

---

# 10. 输出格式设计

支持：

```bash
zpic upload ./cover.png --format markdown
zpic upload ./cover.png --format url
zpic upload ./cover.png --format html
zpic upload ./cover.png --format jsx
```

配置：

```toml
[format]
markdown = "![{alt}]({url})"
html = "<img src=\"{url}\" alt=\"{alt}\" />"
jsx = "<Image src=\"{url}\" alt=\"{alt}\" width={width} height={height} />"
```

这样对 Next.js 用户很有用。

---

# 11. 错误处理设计

错误输出要面向用户，不要只是 Rust panic。

示例：

```text
Upload failed: GitHub token is missing.

Fix:
  1. Set GITHUB_TOKEN environment variable
  2. Or configure token in ~/.config/zpic/config.toml

Run:
  zpic doctor
```

建议错误类型：

```rust
pub enum ZpicError {
    ConfigNotFound,
    ConfigInvalid(String),
    UploaderNotFound(String),
    AuthMissing(String),
    AuthFailed(String),
    FileNotFound(PathBuf),
    UnsupportedFileType(String),
    UploadFailed(String),
    Network(reqwest::Error),
    Io(std::io::Error),
}
```

---

# 12. `zpic doctor` 诊断命令

这是很重要的工程体验。

```bash
zpic doctor
```

输出：

```text
zpic doctor

Config:
  ✓ zpic config found: ~/.config/zpic/config.toml
  ✓ PicGo config found: ~/.picgo/config.json
  ✓ default uploader: r2

Uploader r2:
  ✓ endpoint configured
  ✓ bucket configured
  ✓ access key found from env R2_ACCESS_KEY_ID
  ✓ secret key found from env R2_SECRET_ACCESS_KEY
  ✓ test upload permission

Clipboard:
  ✓ clipboard available

History:
  ✓ history database writable

Result:
  All checks passed.
```

如果失败：

```text
Uploader github:
  ✗ token missing

Fix:
  export GITHUB_TOKEN=ghp_xxx
```

---

# 13. zpic-cli 技术选型

建议依赖：

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
reqwest = { version = "0.12", features = ["json", "multipart", "stream"] }
anyhow = "1"
thiserror = "2"
async-trait = "0.1"
bytes = "1"
directories = "5"
tracing = "0.1"
tracing-subscriber = "0.3"
mime_guess = "2"
infer = "0.16"
blake3 = "1"
uuid = { version = "1", features = ["v4"] }
chrono = "0.4"
arboard = "3"
rusqlite = { version = "0.32", features = ["bundled"] }
```

如果要 S3：

```toml
aws-config = "1"
aws-sdk-s3 = "1"
```

如果要 MCP：

```toml
rmcp = "..."
rmcp-macros = "..."
```

官方 Rust MCP SDK 仓库说明它包含 `rmcp` 和 `rmcp-macros` 两个核心 crate。([GitHub][5])

---

# 14. zpic-cli MVP 开发任务拆分

## Sprint 1：基础 CLI 和配置

任务：

```text
1. 创建 Rust workspace
2. 创建 zpic-core
3. 创建 zpic-cli
4. 实现 clap 命令结构
5. 实现 zpic 原生 TOML 配置读取
6. 实现 PicGo config.json 读取
7. 实现 config import-picgo
8. 实现 doctor 基础检查
```

验收：

```bash
zpic config init
zpic config show
zpic config import-picgo
zpic doctor
```

---

## Sprint 2：Local + GitHub 上传器

任务：

```text
1. 实现 Uploader trait
2. 实现 LocalUploader
3. 实现 GitHubUploader
4. 实现路径模板
5. 实现 Markdown formatter
6. 实现 JSON 输出
7. 实现上传历史 SQLite
```

验收：

```bash
zpic upload ./demo.png --uploader local
zpic upload ./demo.png --uploader github
zpic history list
```

---

## Sprint 3：S3 / R2 上传器

任务：

```text
1. 实现 S3CompatibleUploader
2. 支持 endpoint
3. 支持 region
4. 支持 content-type
5. 支持 cache-control
6. 支持 Cloudflare R2
7. doctor 增加 S3 权限检查
```

验收：

```bash
zpic upload ./demo.png --uploader r2
```

---

## Sprint 4：剪贴板 + Markdown 迁移

任务：

```text
1. 支持 --clipboard
2. 支持 --copy
3. 实现 markdown 本地图片扫描
4. 实现 migrate dry-run
5. 实现 migrate rewrite
6. 输出迁移报告
```

验收：

```bash
zpic upload --clipboard --copy
zpic migrate README.md --dry-run
zpic migrate README.md
```

---

# 15. Zed 插件 zpic 方案

## 15.1 Zed 插件定位

由于 Zed 当前扩展能力和 VS Code 不一样，第一版不要做复杂 UI。

Zed 插件负责：

```text
1. 检测 zpic-cli 是否安装
2. 提供 slash command
3. 调用 zpic-cli
4. 返回 Markdown / URL
5. 后续接入 MCP Server
```

Zed 扩展能力目前支持 Rust 写扩展，编译到 Wasm。官方 Zed 扩展文章也提到扩展新增了语言、主题、snippets、slash commands 等能力。([Zed][8])

## 15.2 extension.toml

```toml
id = "zpic"
name = "zpic"
version = "0.1.0"
schema_version = 1
authors = ["yong zhang"]
description = "Upload images to image hosts using zpic."
repository = "https://github.com/yourname/zpic"

[slash_commands.zpic-upload]
description = "Upload a local image using zpic and return Markdown"
requires_argument = true

[slash_commands.zpic-url]
description = "Upload a local image using zpic and return URL"
requires_argument = true
```

## 15.3 使用方式

在 Zed Assistant：

```text
/zpic-upload ./docs/images/cover.png
```

返回：

```md
![cover](https://cdn.example.com/images/2026/06/04/a8f32d19.png)
```

或者：

```text
/zpic-url ./docs/images/cover.png
```

返回：

```text
https://cdn.example.com/images/2026/06/04/a8f32d19.png
```

## 15.4 插件内部调用

Zed 插件用 `process::Command` 调用：

```bash
zpic upload ./docs/images/cover.png --json
```

然后解析 JSON。

Zed Rust Extension API 文档显示有 `process` 模块，可用于处理进程能力。([文档.rs][4])

## 15.5 Zed 插件限制

要在 README 里明确说明：

```text
当前版本的 Zed 插件主要通过 Slash Command / CLI 工作。
暂不支持像 VS Code 那样监听粘贴图片并自动替换当前编辑器内容。
```

因为 Zed 相关讨论中也有人提到 slash command 当时无法直接与编辑器交互；这个能力边界要提前规避产品预期。([GitHub][9])

---

# 16. MCP Server 方案

## 16.1 MCP Server 定位

`zpic-mcp` 是 AI 工具入口。

运行：

```bash
zpic mcp
```

或者独立：

```bash
zpic-mcp
```

给 AI 提供工具：

```text
upload_image
upload_clipboard_image
migrate_markdown_images
list_upload_history
get_upload_config
doctor
```

---

## 16.2 MCP 工具定义

### upload_image

输入：

```json
{
  "path": "./docs/images/cover.png",
  "uploader": "r2",
  "format": "markdown"
}
```

输出：

```json
{
  "url": "https://cdn.example.com/images/2026/06/04/a8f32d19.png",
  "markdown": "![cover](https://cdn.example.com/images/2026/06/04/a8f32d19.png)",
  "key": "images/2026/06/04/a8f32d19.png"
}
```

### migrate_markdown_images

输入：

```json
{
  "path": "./README.md",
  "dry_run": true
}
```

输出：

```json
{
  "found": 3,
  "uploaded": 0,
  "changes": [
    {
      "from": "./assets/logo.png",
      "to": "https://cdn.example.com/images/2026/06/04/logo.png"
    }
  ]
}
```

---

## 16.3 MCP 安全设计

这个很关键。

MCP Server 默认不要给无限权限。

配置：

```toml
[mcp]
enabled = true
workspace_roots = [
  "/Users/yong/projects"
]
allow_clipboard = false
allow_delete = false
allow_migrate_write = false
require_confirmation_for_delete = true
max_file_size_mb = 20
allowed_extensions = ["png", "jpg", "jpeg", "gif", "webp", "svg"]
```

规则：

```text
1. 只能上传 workspace_roots 内的文件
2. 默认禁止删除远程图片
3. 默认 migrate 只 dry-run
4. 写文件需要显式开启
5. 限制文件大小
6. 限制扩展名
7. 所有工具调用写入审计日志
```

原因是 MCP 的工具调用会被 AI 触发，本地文件和远程上传都属于敏感操作。近期 MCP 生态已经出现关于 STDIO、工具调用和命令执行风险的安全讨论，因此你的 zpic-mcp 要从第一版就设计权限边界。([Tom's Hardware][10])

---

# 17. 项目最终形态

最终你可以形成这样的完整生态：

```text
zpic-core
  Rust SDK，提供配置、上传、格式化、历史、迁移

zpic-cli
  独立命令行工具，兼容 PicGo 配置

zpic-zed
  Zed 插件，通过 slash command / process 调用 zpic-cli

zpic-mcp
  AI Agent 工具服务，支持上传图片、迁移 Markdown、查询历史

zpic-server
  可选，本地 HTTP API，给其他编辑器/工具调用
```

未来还可以扩展：

```text
zpic-obsidian
zpic-typora
zpic-raycast
zpic-alfred
zpic-vscode
```

因为核心能力都在 `zpic-core`。

---

# 18. 推荐版本路线

## v0.1：zpic-cli 可用版

功能：

```text
- Rust workspace
- zpic upload
- zpic config init
- zpic config import-picgo
- 兼容 PicGo GitHub 配置
- local uploader
- github uploader
- markdown/url/html 输出
- --copy
- --json
- doctor
```

## v0.2：图床增强版

功能：

```text
- S3/R2 uploader
- SM.MS uploader
- 上传历史 SQLite
- 路径模板
- 环境变量解析
- token 脱敏
- 多文件上传
```

## v0.3：文档写作增强版

功能：

```text
- clipboard upload
- markdown migrate
- dry-run
- migration report
- image dimension
- duplicate detection
```

## v0.4：Zed 插件

功能：

```text
- zpic-zed extension
- /zpic-upload
- /zpic-url
- 检测 zpic-cli
- 调用 zpic upload --json
```

## v0.5：MCP Server

功能：

```text
- zpic mcp
- upload_image
- migrate_markdown_images
- list_upload_history
- doctor
- workspace 权限限制
```

## v1.0：稳定版

功能：

```text
- GitHub / S3 / R2 / Local / SM.MS / OSS / COS
- PicGo 配置主流兼容
- Markdown 迁移稳定
- Zed 插件可用
- MCP 安全策略完善
- 跨平台安装包
```

---

# 19. 我建议你先做的最小闭环

不要一开始做很多上传器。最小闭环是：

```text
zpic-cli
  ↓
读取 PicGo GitHub 配置
  ↓
上传图片到 GitHub
  ↓
返回 Markdown
  ↓
复制到剪贴板
```

也就是：

```bash
zpic upload ./cover.png --config ~/.picgo/config.json --copy
```

只要这个跑通，就证明：

```text
Rust CLI 可行
PicGo 配置兼容可行
图床上传可行
Markdown 输出可行
后续 Zed/MCP 可复用
```

然后第二步做：

```bash
zpic upload ./cover.png --uploader r2 --copy
```

这样就覆盖免费用户和专业用户。

---

# 20. 最终建议

这个项目最好不要定位成“Zed 图床插件”，而是定位成：

> **Rust 原生 PicGo 替代品 + Zed / MCP / AI 工作流图片基础设施。**

你的路线应该是：

```text
第一步：zpic-cli
第二步：兼容 PicGo 配置
第三步：内置主流 uploader
第四步：Zed 插件
第五步：MCP / Agent 工具
```

技术上完全可行，难点不在 Rust，而在这几个地方：

```text
1. PicGo 配置字段兼容
2. 各图床 API 差异
3. 跨平台剪贴板
4. Zed 当前插件能力边界
5. MCP 安全权限设计
```

我的建议是：**先做 zpic-cli v0.1，只兼容 PicGo GitHub 配置 + local uploader + GitHub uploader + --copy + --json。**
这个版本做出来后，后面的 Zed 插件和 MCP 都只是入口层，不会推倒重来。

[1]: https://docs.picgo.app/core/guide/commands?utm_source=chatgpt.com "CLI Commands"
[2]: https://support.typora.io/Upload-Image/?utm_source=chatgpt.com "Upload Images - Typora Support"
[3]: https://zed.dev/extensions/wc-language-server?utm_source=chatgpt.com "Web Components Language Server Extension"
[4]: https://docs.rs/zed_extension_api/latest/zed_extension_api/?utm_source=chatgpt.com "zed_extension_api - Rust"
[5]: https://github.com/modelcontextprotocol/rust-sdk?utm_source=chatgpt.com "The official Rust SDK for the Model Context Protocol"
[6]: https://arxiv.org/abs/2506.13538?utm_source=chatgpt.com "Model Context Protocol (MCP) at First Glance: Studying the Security and Maintainability of MCP Servers"
[7]: https://github.com/PicGo/vs-picgo?utm_source=chatgpt.com "PicGo/vs-picgo: A VSCode plugin of PicGo"
[8]: https://zed.dev/blog/zed-decoded-extensions?utm_source=chatgpt.com "Life of a Zed Extension: Rust, WIT, Wasm"
[9]: https://github.com/zed-industries/zed/discussions/17403?utm_source=chatgpt.com "The usage of the Slash Command extension #17403"
[10]: https://www.tomshardware.com/tech-industry/artificial-intelligence/anthropics-model-context-protocol-has-critical-security-flaw-exposed?utm_source=chatgpt.com "Anthropic's Model Context Protocol includes a critical remote code execution vulnerability - newly discovered exploit puts 200,000 AI servers at risk"

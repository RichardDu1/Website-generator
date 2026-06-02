# 将 website-generator Skill 部署到服务器版 Hermes

## 前置条件

- 服务器已安装 Hermes Agent 并正常运行（Gateway 后台在线）
- 服务器已安装 Python ≥ 3.10、Node.js ≥ 18
- 你有服务器 SSH 访问权限

---

## 方法一：SCP / SFTP 直传（最简单）

### 1. 从本地上传整个 skill 文件夹

```bash
# 在你的本地 Windows 机器上执行（PowerShell）
# 将 website-generator 整个目录上传到服务器的 Hermes skills 目录

scp -r ./website-generator user@your-server:~/.hermes/skills/website-generator
```

或者用 **SFTP 工具**（如 WinSCP、FileZilla）：

1. 连接到你的服务器
2. 导航到 `~/.hermes/skills/` 目录
3. 将本地的 `website-generator` 整个文件夹拖进去

### 2. 在服务器上确认文件结构

```bash
ssh user@你的服务器IP

# 确认文件到位
ls -la ~/.hermes/skills/website-generator/

# 应该看到：
# SKILL.md                  ← 必须存在，这是 Hermes 识别 Skill 的入口
# scripts/
# design-library/
# references/
```

### 3. 确认 Hermes 已加载

```bash
# 列出所有已安装的 skills
hermes skills list

# 你应该能看到 website-generator 出现在列表中
```

> [!TIP]
> Hermes 会自动扫描 `~/.hermes/skills/` 下的子目录。只要目录里有 `SKILL.md`，它就会被识别为一个 Skill。不需要额外注册或安装命令。

---

## 方法二：Git 仓库（推荐，便于版本管理）

### 1. 先将 skill 推送到 Git 仓库

```bash
# 在本地 website-generator 目录下
cd C:\Users\Administrator\website_skills\website-generator

git init
git add .
git commit -m "website-generator skill v3.0"

# 推送到你的 GitHub（私有仓库也可以）
git remote add origin https://github.com/RichardDu1/website-generator-skill.git
git push -u origin main
```

### 2. 在服务器上克隆

```bash
ssh user@你的服务器IP

cd ~/.hermes/skills/
git clone https://github.com/RichardDu1/website-generator-skill.git website-generator
```

### 3. 后续更新只需 git pull

```bash
cd ~/.hermes/skills/website-generator
git pull
```

---

## 方法三：通过宝塔面板上传

如果你用宝塔面板管理服务器，可以直接通过宝塔的文件管理器操作：

1. 登录宝塔面板 → 文件管理
2. 导航到 `/root/.hermes/skills/`（或你的 Hermes 用户对应的路径）
3. 点击「上传」→ 上传压缩包 → 解压
4. 确认解压后的目录名为 `website-generator`，且 `SKILL.md` 在根层级

> [!WARNING]
> **注意 `.hermes` 是隐藏目录！** 宝塔文件管理器默认不显示隐藏文件/目录。
> 你需要在宝塔文件管理器的地址栏里直接输入路径 `/root/.hermes/skills/` 来访问。

---

## 上传后必做的验证步骤

### 1. 确认文件权限

```bash
# 确保 Hermes 运行用户有读取权限
chmod -R 755 ~/.hermes/skills/website-generator/
```

### 2. 确认 Python 脚本可执行

```bash
cd ~/.hermes/skills/website-generator

# 测试各脚本是否能正常运行
python scripts/seo_engine.py keywords "test"
python scripts/design_intel.py search "test"
python scripts/site_builder.py --help
python scripts/deployer.py --help
python scripts/competitor_intel.py --help
```

### 3. 确认 Hermes 识别了 Skill

```bash
hermes skills list
# 输出中应包含 website-generator
```

### 4. 实际对话测试

在 Telegram/Discord（或 hermes chat）中发送：
```
帮我生成一个关于苏格兰高地旅游的网站
```
Hermes 应该会自动识别并调用 `website-generator` skill 的流程。

---

## 需要排除的文件（可选瘦身）

上传前可以删除以下不必要的文件，减小体积：

```bash
# 删除 Python 字节码缓存
rm -rf scripts/__pycache__/

# 删除测试产物（约 30KB，非必需）
rm -rf test_run/

# 删除对话记录（本地参考用，服务器不需要）
rm -f CONVERSATION-LOG.md
```

瘦身后核心文件仅约 **130KB**，非常轻量。

---

## 完整目录结构（服务器端最终状态）

```
~/.hermes/
├── config.yaml
├── SOUL.md
├── .env
├── memories/
│   ├── MEMORY.md
│   └── USER.md
├── skills/
│   ├── website-generator/          ← 我们上传的 Skill
│   │   ├── SKILL.md                ← Hermes 入口（必须）
│   │   ├── scripts/
│   │   │   ├── __init__.py
│   │   │   ├── competitor_intel.py
│   │   │   ├── seo_engine.py
│   │   │   ├── design_intel.py
│   │   │   ├── site_builder.py
│   │   │   └── deployer.py
│   │   ├── design-library/
│   │   │   └── index.json
│   │   └── references/
│   │       └── tool-registry.md
│   ├── wondelai-skills/            ← 其他已安装的 skills
│   └── .../
├── sessions/
└── logs/
```

---

## 与其他 Skill 的配合安装

我们的 SKILL.md 中引用了一些外部工具和 Skill。在服务器上也建议一并安装：

### SEO 技能（SKILL.md Phase 2 依赖）
```bash
# 这些是 Hermes 社区 skills，按你的实际需求安装
# 如果已经在 wondelai/skills 包里，就不需要重复安装
hermes skills install geo-seo-claude    # 如果有的话
hermes skills install claude-seo
hermes skills install seomachine
```

### 设计抛光工具（SKILL.md Phase 4 依赖）
```bash
# Impeccable — 设计抛光
npx skills add pbakaus/impeccable

# Better Icons — 图标管理（MCP 模式）
npx better-icons setup

# Taste Skill — 高级审美（可选）
npx skills add Leonxlnx/taste-skill
```

### Directus CMS（Phase 4.6 / Phase 5 依赖）
```bash
# 通过 Docker 安装 Directus（宝塔 Docker 管理器也可以）
cd ~/.hermes/skills/website-generator
# deployer.py 会自动生成 docker-compose.yml，到时直接 docker compose up -d
```

---

## 常见问题

### Q: Hermes 找不到 website-generator skill？
**A**: 检查三件事：
1. `SKILL.md` 文件名必须精确（大写 + `.md` 后缀）
2. 文件必须在 `~/.hermes/skills/website-generator/SKILL.md`，不能嵌套多一层目录
3. 如果你用了命名 Profile，skills 目录在 `~/.hermes/profiles/<profile-name>/skills/`

### Q: Python 脚本报 ModuleNotFoundError？
**A**: 我们的脚本只用 Python 标准库（json, re, sys, datetime, pathlib, textwrap），不需要额外 pip install。如果报错，检查 Python 版本是否 ≥ 3.10。

### Q: 上传后需要重启 Hermes 吗？
**A**: 通常不需要。Hermes 在新会话开始时会重新扫描 skills 目录。如果确实没生效，重启 Gateway：
```bash
systemctl restart hermes-gateway
# 或 Docker 方式
docker restart hermes-default
```

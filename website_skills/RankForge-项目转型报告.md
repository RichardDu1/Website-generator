# SEOplant 项目转型报告：AI SEO Website Factory

> **生成日期**: 2026-05-24
> **品牌域名**: seoplant.io ✅ 已注册
> **基于**: Base44-盈利模式分析.md + 开源项目现状 + 市场深度调研
> **目标定位**: Autonomous SEO Site Generator / AI Programmatic SEO Platform

---

## 目录

1. [Executive Summary](#1-executive-summary)
2. [现有项目资产盘点](#2-现有项目资产盘点)
3. [市场全景分析](#3-市场全景分析)
4. [竞品深度拆解](#4-竞品深度拆解)
5. [核心缺口与蓝海机会](#5-核心缺口与蓝海机会)
6. [产品定位与差异化战略](#6-产品定位与差异化战略)
7. [商业模式设计](#7-商业模式设计)
8. [技术架构建议](#8-技术架构建议)
9. [Go-to-Market 策略](#9-go-to-market-策略)
10. [18个月路线图](#10-18个月路线图)
11. [风险与应对](#11-风险与应对)
12. [结论与行动建议](#12-结论与行动建议)

---

## 1. Executive Summary

### 一句话定位

> **输入一个关键词 → AI 自动完成市场分析、竞争对手扫描、SEO 战略生成、网站搭建、程序化内容生成、部署上线、持续监控的完整闭环。**

### 核心发现

| 维度 | 发现 |
|------|------|
| **市场时机** | AI SEO 工具市场 $2.6-2.8B（2025），CAGR 12-19.5%，现在是最佳入局窗口 |
| **竞争格局** | 市场高度碎片化——没有一家做"SEO 大脑 + 网站工厂"的端到端融合 |
| **核心缺口** | AI 写作工具不懂建站，AI 建站工具不懂 SEO，SEO 工具不懂自动执行 |
| **我们的优势** | 已有完整的 5-Phase Pipeline 代码基础（competitor → SEO → design → build → deploy） |
| **最佳定位** | **AI Programmatic SEO Platform** — 不做第 N 个 AI 建站器，做第一个 SEO 工厂 |

### 推荐品牌名

| 选项 | 定位 | 适合场景 |
|------|------|---------|
| **SEOplant** | AI 排名工厂 | B2B SaaS 风格 |
| **SEOplant** | 播种→生长→收获 SEO 流量 | 亲和、易记 |
| **SiteFlow** | 关键词→网站流 | 简洁国际化 |
| **NicheEngine** | 利基市场引擎 | 精准定位 Programmatic SEO |

---

## 2. 现有项目资产盘点

### 2.1 Website-generator 代码库现状

当前代码库包含 **5 个独立 Python 模块**，覆盖网站生成的完整生命周期：

```
website-generator/
├── scripts/
│   ├── competitor_intel.py   ← Phase 1: 竞品智能
│   ├── seo_engine.py         ← Phase 2: SEO/GEO 引擎
│   ├── design_intel.py       ← Phase 3: 设计智能
│   ├── site_builder.py       ← Phase 4: Astro 站点生成
│   └── deployer.py           ← Phase 5: Nginx/Docker 部署
├── references/
│   └── tool-registry.md      ← 工具注册表（GitHub Stars 映射）
├── design-library/
│   └── index.json            ← 设计库索引
└── scottish-highlands-travel/ ← 示例输出站点
```

### 2.2 各模块能力评估

| 模块 | 完成度 | 核心能力 | 待补强 |
|------|--------|---------|--------|
| **competitor_intel.py** | 70% | Jina Search/RSS 竞品发现 → 网页爬取 → SEO 信号分析 → 社交媒体检测 → SWOT 报告生成 | 需要真实 SERP API（DataForSEO/SerpAPI）替代纯 Jina 搜索；需要 Ahrefs/SEMrush 数据源 |
| **seo_engine.py** | 65% | 关键词扩展（100+ 修饰词模板）→ 主题聚类 → 难度评估 → 6 个月内容日历 → GEO 配置（llms.txt + Schema）→ 多语言框架 | 难度评估是纯启发式的（无真实 search volume/KD 数据）；关键词扩展靠模板而非真实搜索数据 |
| **design_intel.py** | 75% | 设计灵感搜索（9 个设计画廊源）→ 设计 Token 提取（颜色/字体/布局/CSS 变量）→ DESIGN.md 生成 → 多设计融合 | 设计 Token 提取准确率受限于 Jina Reader 的 markdown 转换 |
| **site_builder.py** | 60% | Astro 项目脚手架 → 全局 CSS 生成 → SEOHead/Header/Footer 组件 → i18n 路由 | 仅生成静态结构；内容填充仍是手动；缺少程序化页面生成 |
| **deployer.py** | 80% | Nginx 配置（含 SSL/反向代理）→ Docker Compose（Directus/Medusa/Plausible）→ 一键部署脚本 → 隐私分析集成 | 仅适用于自有服务器/VPS；缺少 Vercel/Netlify/Cloudflare 集成 |

### 2.3 现有技术栈

| 层级 | 当前方案 | 评估 |
|------|---------|------|
| **站点框架** | Astro 5.x | ✅ 优秀 — SSG SEO 最强，i18n 原生支持 |
| **内容存储** | Markdown/MDX | ✅ 正确 — AI 生成内容的天然格式 |
| **CMS** | Directus（可选 Docker） | ✅ 自托管，无 vendor lock-in |
| **分析** | Plausible/Tianji | ✅ 隐私友好，无需 cookie banner |
| **部署** | Nginx + 宝塔/Dokploy | ⚠️ 需补充 Vercel/Cloudflare 一键部署 |
| **设计系统** | DESIGN.md 标准 | ✅ 独有优势——从真实网站提取设计 Token |
| **SEO 数据** | 纯启发式（无 API） | ❌ 最大短板——必须接入真实数据源 |
| **AI Agent** | 无（靠 Claude Code 编排） | ⚠️ 需 LangGraph/CrewAI 实现自主 agent |

### 2.4 关键结论

> 代码基础 **方向完全正确**，但当前是"工具集合"而非"产品"。
> 最大的差距不在代码层面，而在 **数据层**（无真实 SEO 数据源）和 **产品化**（无 SaaS 界面、无付费体系、无自主 Agent）。

---

## 3. 市场全景分析

### 3.1 市场规模

| 报告来源 | 市场定义 | 2025 规模 | 2032 预测 | CAGR |
|---------|---------|----------|----------|------|
| QYResearch | AI SEO 软件工具 | **$2.80B** | $6.2B | 12.0% |
| YH Research | AI 驱动型 SEO 工具 | **$2.77B** | $6.7B | 14.1% |
| MarketIntelo | AI-Native SEO 平台（窄口径） | **$1.20B** | $6.4B | **19.5%** |
| 360iResearch | AI SEO 工具（宽口径） | **$19.35B** | $54.4B | 15.9% |
| Sparkco | Programmatic SEO（SOM） | **$2-5B** | $5B (2027) | 25-40% |

### 3.2 市场趋势（2026）

1. **Google AI Overviews 覆盖 46%+ US SERPs** → 传统 SEO 流量被侵蚀，但 AI 引用成为新流量源
2. **GEO（Generative Engine Optimization）成为独立品类** — 追踪品牌在 ChatGPT、Perplexity、Gemini 中的曝光
3. **LLM API 成本暴跌 80%** → programmatic content 规模化生产成本急剧下降
4. **65% 企业正在集成 AI SEO**（Forrester Q1 2025）
5. **自主 SEO Agent 早期采用者报告 28-41% 有机流量增长**（6 个月内）

### 3.3 客户细分与支付能力

| 客户群 | 规模 | 月支付意愿 | 痛感 |
|--------|------|-----------|------|
| **Affiliate Marketer（单人）** | 巨大 | $29-99/月 | 需要快速建站+SEO 排名，技术能力弱 |
| **SEO Agency（5-20 人）** | 中等 | $199-499/月 | 需要批量建站、白标报告、客户管理 |
| **Local SEO 公司** | 大 | $99-299/月 | 需要批量城市/区域页面 |
| **SaaS 初创公司** | 中 | $99-299/月 | 需要产品对比页、博客、文档 SEO |
| **内容农场/Media 公司** | 小但高价值 | $999+/月 | Programmatic SEO 大规模页面生成 |
| **电商独立站** | 大 | $49-149/月 | 产品页面、类目页面 SEO |

> **最佳初始目标**: Affiliate marketers + SEO agencies——技术能力弱、直接 ROI 可衡量、流失率低。

---

## 4. 竞品深度拆解

### 4.1 竞品分类矩阵

我们将 25+ 竞品划分为 **4 个象限**：

```
                    AI 建站能力强 ↑
                         |
        Durable       10Web        【我们瞄准的位置】
        Wix AI        Hostinger    ★ AI SEO Website Factory
        Framer AI     Relume       （目前无人占据）
                         |
    ──────────────────────────────────────→ SEO 能力强
                         |
        Base44        Byword       Surfer SEO
        Lovable       Frase        Ahrefs
        Bolt.new      MarketMuse   SEMrush
        Replit        SEOmatic     Clearscope
                         |
```

### 4.2 第一象限：AI 建站工具（SEO 能力弱）

<details>
<summary>展开详细分析</summary>

#### Durable.co
| 维度 | 评价 |
|------|------|
| **定位** | 30 秒 AI 生成小企业网站 |
| **定价** | $15-95/月 |
| **优势** | 速度极快、内置 CRM+发票、Cloudflare CDN |
| **劣势** | 设计千篇一律、SEO 极弱（无 Schema/无高级 SEO 控制）、无法导出代码 |
| **与我们关系** | **不直接竞争**——他们是"快速建站"，我们是"SEO 建站" |
| **启示** | 他们的"速度"是卖点，我们可以做到"SEO 速度"作为差异化 |

#### 10Web.io
| 维度 | 评价 |
|------|------|
| **定位** | AI + WordPress Hosting |
| **定价** | $10-60/月（含 hosting） |
| **优势** | Google Cloud hosting、WordPress 生态、Agency 白标 |
| **劣势** | 本质是 Hosting 公司、AI 只是获客工具、WordPress 架构限制 |
| **与我们关系** | **互补大于竞争**——他们卖 hosting，我们卖 SEO 工厂 |
| **启示** | Hosting 是高续费率收入来源，我们的 deployer 模块可做类似变现 |

#### Relume.io
| 维度 | 评价 |
|------|------|
| **定位** | AI 网站线框图 + 组件库（Webflow/Figma） |
| **定价** | $38-79/月 |
| **优势** | 极快的网站结构生成、Webflow 组件生态 |
| **劣势** | 不做 SEO 战略、不做内容生成、依赖 Webflow |
| **与我们关系** | 可以集成——Relume 做结构，我们做 SEO 层 |

</details>

### 4.3 第二象限：AI SEO/内容工具（建站能力无）

<details>
<summary>展开详细分析</summary>

#### Byword.ai
| 维度 | 评价 |
|------|------|
| **定位** | AI 批量 SEO 文章生成 + Programmatic SEO Suite |
| **定价** | $99-1,999/月 + $3,999+/月托管方案 |
| **优势** | 批量内容生产、Programmatic SEO 模板引擎、自动内链、GSC 集成 |
| **劣势** | **不会建站**——文章生成了还需自己发布到网站；无竞品分析；难以解决"什么内容该写"的问题 |
| **与我们关系** | **最接近的竞品之一**——但他们是"内容引擎"，我们是"网站工厂" |
| **启示** | 我们的 Programmatic SEO Engine 应直接对标 Byword 的模板系统 |

#### Surfer SEO
| 维度 | 评价 |
|------|------|
| **定位** | On-page SEO 内容优化（NLP 评分） |
| **定价** | $79-219/月 |
| **优势** | 实时 NLP 内容评分、SERP 分析器、AI 内容编辑器 |
| **劣势** | **不生成网站、不做关键词策略、不自动发布**——只是"编辑器里的评分" |
| **与我们关系** | 我们的 SEO 评分引擎可对标，但他们不懂建站 |
| **启示** | 实时内容评分是好的功能方向，但不应该是产品核心 |

#### Frase.io
| 维度 | 评价 |
|------|------|
| **定位** | AI 内容研究 + 草稿生成 |
| **定价** | $45-299/月 |
| **优势** | SERP 研究→大纲→草稿快速工作流、品牌声音定制、性价比高 |
| **劣势** | **不含关键词排名追踪、不含网站生成、不含自动发布** |
| **与我们关系** | 内容生成层的竞品——但我们的差异化是"生成+建站+部署"一体 |

#### MarketMuse
| 维度 | 评价 |
|------|------|
| **定位** | NLP 驱动的 Topical Authority 规划 |
| **定价** | $149-999+/月（定制） |
| **优势** | Topic Modeling 深度最强、内容差距分析、权威度评分 |
| **劣势** | 极度昂贵、只有分析没有执行、学习曲线陡峭 |
| **与我们关系** | 我们的 "SEO Strategy Engine" 应对标 MarketMuse 的 Topical Map |
| **启示** | Topical Authority 是护城河——MarketMuse 证明企业愿意为此付高价 |

#### SEOmatic.ai
| 维度 | 评价 |
|------|------|
| **定位** | AI 驱动的 Programmatic SEO 页面生成 |
| **定价** | $99-499/月 |
| **优势** | 模板+数据集→批量页面、WordPress/Webflow 集成 |
| **劣势** | 依赖 WordPress、无竞品分析、无自主建站 |
| **与我们关系** | 最直接的 Programmatic SEO 竞品——但他们不建站 |

</details>

### 4.4 第三象限：AI App Builder（不相关但值得关注）

<details>
<summary>展开详细分析</summary>

| 工具 | 定位 | 月费 | 为什么不是竞品 |
|------|------|------|---------------|
| **Base44** | Prompt → Web App | $29-160 | 做 SaaS 应用，不做内容站 |
| **Lovable** | AI 全栈应用生成 | $20-100 | 做 App，不做 SEO 网站 |
| **Bolt.new** | 浏览器内 AI 全栈开发 | $20-200 | 开发者工具，不是营销工具 |
| **Replit Agent** | AI 驱动的在线 IDE | $25-250 | 面向开发者，不面向营销人 |
| **v0.dev** | AI UI 组件生成 | $20-200 | 只生成 UI 组件，不成网站 |

> **这些都不是竞品**——他们面向开发者做应用，我们面向营销人做流量。

</details>

### 4.5 第四象限：数据/API 基础设施（赋能者，非竞品）

<details>
<summary>展开详细分析</summary>

| 工具 | 核心能力 | 月费（起步） | 与我们关系 |
|------|---------|------------|-----------|
| **DataForSEO** | SERP 数据、关键词数据、竞争对手数据 | $50-500 | **必须集成**——性价比最高的 SEO 数据 API |
| **SerpAPI** | Google SERP 抓取 | $50-250 | **备选数据源** |
| **SEMrush API** | 关键词 + 反向链接数据 | 企业定制 | 高端选项，适合 Agency 版 |
| **Ahrefs API** | 反向链接 + 关键词难度 | 企业定制 | 同上，数据质量最高但最贵 |
| **ScrapingBee** | 网页抓取代理 | $49-299 | **爬取层**——竞品网站分析的基础设施 |
| **Jina AI** | Reader + Search API | 免费额度 | 当前在用，可作为免费 tier 数据源 |
| **OpenAI/Claude API** | LLM 核心引擎 | 按量付费 | **AI 成本核心**——需 credits 定价对冲 |

</details>

---

## 5. 核心缺口与蓝海机会

### 5.1 行业最大缺口

通过分析 25+ 竞品，我们识别出 **5 个系统性缺口**：

```
缺口 #1: "分析与执行"断层
  ├─ Surfer/Frase/MarketMuse 能分析但不能建站
  ├─ Byword 能写文章但不能建站
  ├─ 10Web/Durable 能建站但不懂 SEO
  └─ 没有一家连接"SEO 战略→网站执行"

缺口 #2: "Programmatic SEO"孤岛
  ├─ SEOmatic 能做 pSEO 但依赖 WordPress
  ├─ PageFactory/Typemat 能生成页面但无 SEO 智能
  └─ 没有一家做"智能 pSEO + 自主建站"

缺口 #3: "持续优化"真空
  ├─ 现有工具要么生成（一次性）要么分析（不执行）
  ├─ 没有人做到"网站自动增长引擎"
  └─ 自主 Agent 监控→更新→扩展→修复的全闭环是真空

缺口 #4: "多语言 SEO"荒漠
  ├─ 现有工具对多语言支持极弱（多数英文 only）
  ├─ 跨国 SEO 需要独立的关键词研究（不能直接翻译）
  └─ 我们在 seo_engine.py 已实现"独立研究框架"

缺口 #5: "GEO + 传统 SEO"融合
  ├─ GEO 工具（Gauge/Profound/Rankscale）专注 AI 搜索追踪
  ├─ 传统 SEO 工具（Ahrefs/SEMrush）专注 Google
  └─ 没有平台同时优化 Google + ChatGPT + Perplexity + Gemini
```

### 5.2 我们的"蓝海"定位

```
┌──────────────────────────────────────────────────┐
│                                                  │
│   传统 SEO 工具        AI SEO 内容工具           │
│   (Ahrefs, SEMrush)    (Surfer, Frase, Byword)   │
│   ✅ 数据强              ✅ AI 能力强              │
│   ❌ 不会建站            ❌ 不会建站                │
│   ❌ 不会自动执行        ❌ 不会自动执行            │
│                                                  │
├──────────────────────────────────────────────────┤
│                                                  │
│   AI 建站工具           ★ 我们 ★                  │
│   (10Web, Durable)     AI SEO Website Factory    │
│   ✅ 建站快              ✅ SEO 战略 + 执行一体化   │
│   ❌ SEO 弱              ✅ 竞品分析 + 市场定位     │
│   ❌ 内容策略无          ✅ Programmatic SEO 引擎  │
│                         ✅ 自动建站 + 部署         │
│                         ✅ 持续自主增长            │
│                                                  │
└──────────────────────────────────────────────────┘
```

### 5.3 为什么现在？（时机窗口）

| 因素 | 状态 | 含义 |
|------|------|------|
| LLM API 成本 | 暴跌 80%（2024-2025） | Programmatic SEO 规模生产成本可承受 |
| Google AI Overviews | 覆盖 46%+ SERPs | GEO 需求爆发，新品类窗口 |
| AI 建站竞争 | 极度同质化 | 差异化窗口——没有人做"SEO-first" |
| 开源 LLM | DeepSeek/Qwen 崛起 | 可以自部署降低 AI 成本 |
| Programmatic SEO | 从灰色→主流的转折点 | 企业级需求正在形成 |

---

## 6. 产品定位与差异化战略

### 6.1 核心定位

> **AI Programmatic SEO Platform — 从关键词到排名的全自动工厂**

不是：

- ❌ "AI Website Builder"（太拥挤、同质化、OpenAI 一个功能就能淘汰你）
- ❌ "AI SEO Tool"（只分析不执行，用户需要手动建站）
- ❌ "AI Content Writer"（Jasper/Copy.ai 已经在卷价格）

而是：

- ✅ "输入 niche → 输出排名网站"的完整闭环
- ✅ SEO 战略引擎 + 网站工厂 + 持续增长 Agent 三合一

### 6.2 核心卖点（USP）

| # | 卖点 | 一句话 | 对标竞品差距 |
|---|------|--------|------------|
| **1** | **SEO 大脑** | 自动分析竞品 → 制定 SEO 战略 → 生成 Topical Map | Surfer/Frase 只能分析不能建站 |
| **2** | **工厂模式** | 一个关键词 → 数百个 SEO 页面同时生成 | Byword 只能生成文章不能生成网站 |
| **3** | **零操作建站** | 不需要 WordPress/Wix/Webflow——我们自带 Astro SSG | Durable 建站但不做 SEO |
| **4** | **持续增长 Agent** | 排名监控 → 内容更新 → 关键词扩展 → 自动修复 | 没有竞品做到这个闭环 |
| **5** | **多语言原生** | 独立多语言关键词研究，非翻译式多语言 | MarketMuse/Frase 英文 only |
| **6** | **开源 + 可导出** | 网站代码完整导出，无 Vendor Lock-in | Base44/Durable/Wix 锁死用户 |
| **7** | **GEO 内置** | llms.txt + Schema + AI 引用优化 | 传统 SEO 工具只有 Google 维度 |

### 6.3 差异化护城河

| 层级 | 护城河类型 | 具体内容 | 可复制性 |
|------|-----------|---------|---------|
| **短期** | 数据飞轮 | 每生成一个网站就积累该 niche 的 SEO 知识图谱 | 中（需要时间） |
| **中期** | 流程自动化 | SEO Strategy Engine + Site Factory 的完整 Pipeline | 低（竞品可复制功能） |
| **长期** | 网络效应 | 设计库、内容模板、行业模板的社区贡献 | 高（Once established） |
| **终极** | 自主 Agent | AI 驱动的网站持续增长——建站只是开始 | 最高（需要数据和经验积累） |

---

## 7. 商业模式设计

### 7.1 收入模型：三层金字塔

参考 Base44（Credits 制）+ 10Web（Hosting 制）的混合模型：

```
        ┌──────────────────────┐
        │   Enterprise/Agency  │  $499-1,999/月
        │   白标 · API · 私有部署   │
        │   ↑ 高利润，低数量        │
        ├──────────────────────┤
        │   Pro/Growth         │  $79-199/月
        │   无限站点 · pSEO · Agent│
        │   ↑ 核心收入来源         │
        ├──────────────────────┤
        │   Starter/Free       │  $0-29/月
        │   1个站点 · 基础SEO · 获客 │
        │   ↑ 转化漏斗入口         │
        └──────────────────────┘
```

### 7.2 详细定价体系

| 方案 | 月费 | 年费 | 站点数 | AI Credits/月 | 核心功能 |
|------|------|------|--------|--------------|---------|
| **Free** | $0 | - | 1 | 100 | 基础竞品分析 + SEO 蓝图 + 1页站点预览 |
| **Starter** | $29 | $24 | 3 | 500 | 完整 SEO 计划 + Astro 站点生成 + 部署 + 基础 pSEO（50 页） |
| **Pro** | $79 | $69 | 10 | 2,000 | 无限 pSEO 页面 + 自主 Agent 监控 + 设计融合 + 多语言 |
| **Agency** | $199 | $179 | 50 | 5,000 | 白标报告 + 客户管理面板 + API 访问 + 优先支持 |
| **Enterprise** | $999+ | 定制 | 无限 | 无限 | 私有部署 + 专属模型 + SLA + 托管服务 |

### 7.3 附加收入流

| 收入流 | 描述 | 预估贡献 |
|--------|------|---------|
| **AI Credits 充值** | 超出月额度的按量付费 | 15-20% 总收入 |
| **Hosting 托管** | 一键部署到我们的服务器（类似 10Web） | 10-15% 总收入 |
| **模板市场** | 社区/官方设计的 niche 模板、内容模板 | 5% 收入（后期） |
| **API 访问** | DataForSEO 数据通过我们的 API 转发（加价） | 5-10% 收入 |
| **白标许可** | Hosting 公司/Agency 集成我们的引擎 | 高利润（后期） |
| **托管服务** | 完全代运营（类似 Byword $3,999/月） | 高客单价 |

### 7.4 单位经济学（Pro 用户为例）

| 项目 | 月成本 |
|------|--------|
| LLM API（2,000 credits ≈ 100 篇文章 + 50 pSEO 页） | ~$15-25 |
| SEO 数据 API（DataForSEO，按调用量） | ~$5-10 |
| 爬虫基础设施（ScrapingBee/Jina） | ~$5 |
| 服务器/部署（Vercel 免费 tier / VPS） | ~$3-8 |
| **总成本/用户** | **~$28-48** |
| Pro 月费 | $79 |
| **毛利率** | **~40-65%** |

### 7.5 盈亏平衡预测

| 节点 | 用户数 | 月收入（假设 ARPU $79） | 月成本 | 状态 |
|------|--------|------------------------|--------|------|
| MVP 上线 | 0 | $0 | ~$500（服务器+API 测试） | 烧钱 |
| PMF 验证 | 50 | $3,950 | ~$1,500 | 接近打平 |
| 初期增长 | 200 | $15,800 | ~$5,000 | 盈利 |
| 规模化 | 1,000 | $79,000 | ~$20,000 | 健康盈利 |

---

## 8. 技术架构建议

### 8.1 推荐技术栈

| 层级 | 推荐方案 | 理由 |
|------|---------|------|
| **前端框架** | Next.js 15 (App Router) | 比 Astro 更适合 SaaS 后台 + SSG 双模式 |
| **站点生成** | **保留 Astro 5.x**（输出端） | Astro SEO 最强——生成的客户站点用 Astro |
| **AI Agent** | LangGraph + LangChain | 多 Agent 编排——竞品分析 Agent → SEO Agent → 内容 Agent |
| **LLM** | Claude API（主力）+ DeepSeek（降本） | Claude 内容质量最高，DeepSeek 成本极低 |
| **数据库** | Supabase (PostgreSQL) | 用户数据 + 项目管理，自带 auth |
| **SEO 数据** | **DataForSEO API**（核心）+ SerpAPI（备选） | DataForSEO 性价比最高，$50/月起 |
| **爬虫** | ScrapingBee + Jina Reader | 竞品网站分析 + 内容提取 |
| **部署输出** | Vercel + Netlify + Cloudflare Pages | 支持一键部署到多个平台 |
| **分析** | Plausible（自托管） | 隐私友好，内置在 deployer 中 |
| **支付** | Stripe + LemonSqueezy | Stripe 主力，LemonSqueezy 处理全球税务 |

### 8.2 核心架构图

```
用户输入: "best hiking boots affiliate site"
              │
              ▼
    ┌─────────────────────┐
    │   Market Intel Agent │  ← DataForSEO + Jina Search
    │   市场分析 · 竞品发现  │
    └─────────┬───────────┘
              │ 竞品列表 + SERP 数据
              ▼
    ┌─────────────────────┐
    │   SEO Strategy Agent │  ← Claude API + DataForSEO
    │   Topical Map · 内容日历│
    └─────────┬───────────┘
              │ 关键词集群 + 页面结构
              ▼
    ┌─────────────────────┐
    │   Design Intel Agent │  ← Jina Reader + CSS 提取
    │   设计 Token · DESIGN.md│
    └─────────┬───────────┘
              │ 设计系统
              ▼
    ┌─────────────────────┐
    │   Site Factory       │  ← Astro 5.x SSG
    │   页面生成 · Schema · SEO│
    └─────────┬───────────┘
              │ 完整 Astro 项目
              ▼
    ┌─────────────────────┐
    │   Content Engine     │  ← Claude API + pSEO 模板
    │   批量内容 · pSEO 页面 │
    └─────────┬───────────┘
              │ 填充内容的网站
              ▼
    ┌─────────────────────┐
    │   Deploy & Monitor   │  ← Vercel/GitHub Actions
    │   一键部署 · 持续监控   │
    └─────────────────────┘
              │
              ▼
    🚀 Live Website + Continuous SEO Agent
```

### 8.3 数据层——最大投入

这是最关键的决策。SEO 数据是护城河，但也是最烧钱的部分。

| 数据需求 | 推荐方案 | 月成本（最小规模） | 替代方案 |
|---------|---------|-----------------|---------|
| SERP 数据 | DataForSEO SERP API | $50-100 | SerpAPI $50 |
| 关键词数据 | DataForSEO Keyword Data | 含在 SERP API 中 | SEMrush API（贵） |
| 竞品抓取 | ScrapingBee + Jina Reader | $50-100 | 自建爬虫 |
| 反向链接 | 初期不做 | $0 | Ahrefs API 后期集成 |
| AI 内容生成 | Claude API + DeepSeek | $200-500（按使用量） | GPT-4o 更贵 |
| **月总数据成本（MVP）** | | **$300-700** | |

---

## 9. Go-to-Market 策略

### 9.1 核心策略：Product-Led Growth + 开源社区

我们已经有 GitHub 开源仓库，这是最大的早期获客资产。

### 9.2 获客渠道矩阵

| 渠道 | 优先级 | 策略 | 预期获客成本 |
|------|--------|------|------------|
| **GitHub 开源** | ⭐⭐⭐⭐⭐ | 完善 README、Showcase、文档；在 Hacker News/Reddit 推广 | $0（时间成本） |
| **SEO 自身** | ⭐⭐⭐⭐⭐ | 用我们自己的产品建 SEO 内容站——"吃自己的狗粮" | $0 |
| **Reddit 社区** | ⭐⭐⭐⭐ | r/seo, r/juststart, r/affiliatemarketing, r/saas | $0（时间成本） |
| **Product Hunt 发布** | ⭐⭐⭐⭐ | 每个大版本发布一次，积累种子用户 | $0（需精心策划） |
| **SEO 工具对比文章** | ⭐⭐⭐⭐ | "Byword vs Surfer vs [我们]" 对比页 | $0（内容成本） |
| **YouTube SEO 教程** | ⭐⭐⭐ | 展示从关键词到网站的完整工作流 | $0（时间成本） |
| **Indie Hackers / X** | ⭐⭐⭐ | 公开 Build in Public 进度 | $0 |
| **付费广告** | ⭐⭐ | 后期：Google Ads（关键词："programmatic SEO tool"） | $50-200/转化 |

### 9.3 早期种子用户获取（前 100 个付费用户）

| 方法 | 目标 | 转化路径 |
|------|------|---------|
| Free Tier 免费建站 | 让用户先用免费方案生成网站 | 限制站点数/页面数 → 需要更多就付费 |
| 开源社区贡献者折扣 | GitHub Star/PR 贡献者获得 Pro 折扣 | Star → 免费方案 → 付费 |
| SEO Agency 试点合作 | 找 5 家 SEO agency 免费试用 2 个月 | 白标 + 批量建站 → 年付锁定 |
| 竞品不满用户 | 在 Reddit/G2 找对 Byword/Frase/Durable 不满的用户 | 提供迁移方案 |

### 9.4 品牌叙事

```
"我们不帮你建网站。我们帮你建流量机器。"

From: "Build a website"     →  To: "Build a traffic engine"
From: "AI writes content"   →  To: "AI runs your SEO"
From: "Single site"         →  To: "Site factory"
```

---

## 10. 18 个月路线图

### Phase 1: SEO 大脑 MVP（第 1-3 月）

**目标**: 验证"AI 能做 SEO 战略"的核心假设

| 任务 | 详情 | 交付物 |
|------|------|--------|
| 集成 DataForSEO API | 替代纯启发式关键词分析 | 真实 search volume + KD 数据 |
| SEO Strategy Engine v1 | 输入关键词→输出完整 SEO 蓝图（Topical Map + 内容日历） | Web 界面 + Markdown 导出 |
| 竞品分析报告 v1 | 自动抓取 5 个竞品 + SWOT 表格 | 付费报告（$9/份）验证付费意愿 |
| Free Tier 上线 | 1 个关键词分析免费 | 收集邮件 + 验证需求 |

**关键指标**:
- 100 个免费用户注册
- 20 份付费报告售出
- NPS > 40

### Phase 2: 网站工厂 Beta（第 4-6 月）

**目标**: 完成"SEO 蓝图→网站生成"闭环

| 任务 | 详情 | 交付物 |
|------|------|--------|
| Site Factory v1 | 根据 SEO 蓝图自动生成 Astro 网站 | 完整网站代码生成 |
| Content Engine v1 | 批量生成 50-100 页内容 | 基于 SEO 蓝图的自动填充 |
| 一键部署 | Vercel + Netlify 集成 | 从生成到上线 < 5 分钟 |
| 付费墙 | Stripe 集成 + Credits 系统 | Starter/Pro 方案上线 |

**关键指标**:
- 50 个付费用户
- 网站生成→部署成功率 > 90%
- MRR $2,000+

### Phase 3: Programmatic SEO 引擎（第 7-9 月）

**目标**: 爆发式增长——一个关键词生成数百页

| 任务 | 详情 | 交付物 |
|------|------|--------|
| pSEO Template Engine | 模板 + 数据集 → 批量页面 | 对标 Byword Programmatic Suite |
| 多语言 v1 | 独立关键词研究 + 翻译框架 | 5 种语言支持 |
| Design Library 公开 | 社区贡献设计 Token 库 | 设计参考平台 |
| Affiliate 集成 | 自动插入 Amazon/ShareASale 链接 | Affiliate 网站专用模板 |

**关键指标**:
- 200 个付费用户
- pSEO 页面生成量 > 10,000 页/月
- MRR $10,000+

### Phase 4: 自主 SEO Agent（第 10-14 月）

**目标**: 从"工具"到"Agent"的质变

| 任务 | 详情 | 交付物 |
|------|------|--------|
| SEO Monitor Agent | 自动监控排名变化 + GSC 数据 | 每周自动报告 |
| Content Refresh Agent | 检测排名下降 → 自动更新内容 | 一键内容更新 |
| Keyword Expansion Agent | 发现新关键词 → 自动扩展 cluster | 自动内容扩展 |
| Cannibalization Fix | 检测关键词自相残杀 → 自动修复 | 内链优化 |

**关键指标**:
- 500 个付费用户
- Agent 自动更新采用率 > 60%
- MRR $30,000+
- Churn < 5%

### Phase 5: 平台生态（第 15-18 月）

**目标**: 从 SaaS 到平台

| 任务 | 详情 | 交付物 |
|------|------|--------|
| Agency 白标 | 自定义品牌 + 客户管理面板 | 企业版上线 |
| API 开放 | REST API + Webhook | 第三方集成 |
| 模板市场 | 社区/官方模板交易 | 生态启动 |
| 企业私有部署 | Docker + K8s 部署包 | Enterprise 方案 |

**关键指标**:
- 1,000+ 付费用户
- 10 个 Agency 白标客户
- MRR $80,000+
- 年度 ARR $1M+ 里程碑

---

## 11. 风险与应对

| # | 风险 | 严重性 | 概率 | 应对策略 |
|---|------|--------|------|---------|
| 1 | **OpenAI/Google 推出类似功能** | 高 | 中 | 不做"通用 AI 建站"——专注 SEO 工作流自动化，这是大公司不会做的垂直深度 |
| 2 | **AI 内容被 Google 惩罚** | 高 | 中 | 内置 E-E-A-T 质量闸门 + 人工审核标记 + 内容差异化算法 |
| 3 | **LLM API 成本失控** | 中 | 高 | Credits 定价对冲 + DeepSeek 等低成本模型降本 + 自部署开源模型 |
| 4 | **Google AI Overviews 消灭 SEO** | 高 | 低 | 同时做 GEO（AI 引用优化）——即使 Google 流量下降，ChatGPT/Perplexity 流量在增长 |
| 5 | **用户不做 SEO 维护导致效果差** | 中 | 高 | Phase 4 的自主 Agent 就是解决这个问题——让 AI 做维护，不依赖用户 |
| 6 | **竞争对手复制功能** | 中 | 高 | 速度 + 数据飞轮 + 开源社区——功能可复制，数据和生态不可复制 |
| 7 | **DataForSEO 等 API 涨价** | 低 | 中 | 多数据源策略（至少 2 个备选），自建爬虫 fallback |
| 8 | **开源版 cannibalize 付费版** | 中 | 中 | 开源版限制站点数 + 无 Agent + 不支持 pSEO → 付费版提供"工厂级"能力 |

---

## 12. 结论与行动建议

### 12.1 核心结论

1. **方向正确性: 9/10** — 你是对的，没人做"SEO 战略 + 网站工厂"的融合
2. **时机: 8/10** — AI SEO 工具市场正在爆发，但窗口期可能在 12-18 个月
3. **竞争风险: 4/10** — 虽然玩家多，但没有人占据"SEO 工厂"这个定位
4. **执行可行性: 7/10** — 代码基础好，但需要投入数据 API 成本和 SaaS 产品化

### 12.2 本周应立即行动的事项

| # | 行动 | 优先级 | 预计时间 |
|---|------|--------|---------|
| 1 | **确定品牌名** — SEOplant / SEOplant / NicheEngine 三选一 | 🔴 P0 | 1 天 |
| 2 | **注册 DataForSEO 账号** — $50/月起步，开始接入真实数据 | 🔴 P0 | 2 天 |
| 3 | **搭建 Landing Page** — 用你自己的工具建一个产品官网 | 🔴 P0 | 3 天 |
| 4 | **写一篇 "Why AI Website Builders Fail at SEO" 的博客** — 定义品类 | 🟡 P1 | 1 天 |
| 5 | **在 r/seo 和 r/saas 发布开源项目** — 获取早期反馈 | 🟡 P1 | 1 天 |
| 6 | **整合 DataForSEO → seo_engine.py** — 替代启发式分析 | 🟡 P1 | 3 天 |

### 12.3 90 天冲刺目标

> **90 天内：让一个人愿意为"AI 生成的 SEO 蓝图"付费 $9**

这不是一个技术里程碑，而是一个 **商业验证里程碑**。如果 90 天内没有人愿意付 $9 买 SEO 蓝图，那么 SEO 工厂也不需要建——因为没有 PMF。

### 12.4 最终产品愿景

> 2027 年的某一天：
>
> 一个 affiliate marketer 登录 SEOplant，输入 "best running shoes for flat feet"。
>
> AI 自动：
> - 扫描 20 个竞品网站，分析他们的 URL 结构、关键词、内容策略
> - 生成一个 50 个关键词的 Topical Map
> - 标注哪些是高商业价值的 buyer-intent 关键词（买鞋），哪些是 informational（如何判断扁平足）
> - 自动搭建一个 Astro 网站，包含分类页面、产品对比页面、博客页面、FAQ 页面
> - 生成 200 页 programmatic SEO 内容（每个品牌 × 每个鞋型 × 每个足型）
> - 部署到 Cloudflare，配置好 Schema、llms.txt、sitemap
> - 注册 Google Search Console，提交索引
>
> **全程用户只输入了一个关键词。**
>
> 这不是 AI 建站器。这是 AI 网站工厂。

---

## 附录

### A. 关键竞品定价总览

| 竞品 | 类型 | 入门价 | 中间价 | 高端价 |
|------|------|--------|--------|--------|
| Byword | AI SEO 内容 | $99/月 | $299/月 | $1,999/月 |
| Surfer SEO | On-page SEO | $79/月 | $219/月 | 定制 |
| Frase | AI 内容优化 | $45/月 | $129/月 | $299/月 |
| MarketMuse | Topical Authority | $149/月 | $999/月 | 定制 |
| Durable | AI 建站 | $15/月 | $25/月 | $95/月 |
| 10Web | AI WP Hosting | $10/月 | $30/月 | $60/月 |
| SEOmatic | pSEO | $99/月 | $199/月 | $499/月 |
| Relume | AI 线框图 | $38/月 | $79/月 | 定制 |
| **我们（建议）** | **AI SEO Factory** | **$29/月** | **$79/月** | **$199-999/月** |

### B. 开源 vs 竞品优势对比

| 维度 | 我们（开源） | Byword | Surfer | Durable | 10Web |
|------|------------|--------|--------|---------|-------|
| 竞品分析 | ✅ 自动 | ❌ | ❌ | ❌ | ❌ |
| SEO 战略生成 | ✅ AI Agent | ❌ | ❌ | ❌ | ❌ |
| 网站生成 | ✅ Astro SSG | ❌ | ❌ | ✅ 模板 | ✅ WP |
| 代码可导出 | ✅ 完全导出 | ❌ | ❌ | ❌ | ⚠️ WP |
| Programmatic SEO | ✅ 模板引擎 | ✅ 模板 | ❌ | ❌ | ❌ |
| GEO 优化 | ✅ 内置 | ❌ | ⚠️ 部分 | ❌ | ❌ |
| 多语言独立研究 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 自主持续优化 | 🔜 Phase 4 | ❌ | ❌ | ❌ | ❌ |
| Vendor Lock-in | ✅ 无 | ⚠️ 部分 | ❌ | ❌ | ⚠️ |
| 开源 | ✅ MIT | ❌ | ❌ | ❌ | ❌ |

---

> **报告完成。下一步：确定品牌名 + 注册 DataForSEO + 搭建 Landing Page。**

---

## 附录 C：深度调研补充（2026-05-24 第二轮）

> 以下内容来自第二轮专门竞品调研，补充了 22+ 竞品的详细定价、新兴端到端竞品、6 大结构性缺口和更精确的数据成本分析。

### C.1 新兴端到端竞品（最值得关注的早期玩家）

有一批早期产品正在试图打通"AI 内容 + 自动化"的链路。这些比传统 SEO 工具更接近我们的概念：

| 产品 | 做什么 | 价格 | 缺口 |
|------|--------|------|------|
| **Quattr GIGA** | AI landing page 生成器，含关键词验证、竞品分析、内容审计、可直接部署的 HTML | 企业定制（非公开） | 仅 landing page，不做完整站点 |
| **RankSpot** | 每日 AI 博客文章自动发布 + 竞品关键词追踪 + LLM 优化 | $39/月 | 仅博客内容，不建站 |
| **SEO45 AI** | 多 Agent 内容引擎，每日一篇带图博客 | 未公开 | 仅博客，不建站 |
| **Keytomic** | All-in-one AI SEO Agent：关键词发现 + 内容 + 技术 SEO + 外链 + AI 可见度 | 未公开 | 内容+监控，不建站 |
| **AymarTech** | 全周期 AI 博客：关键词研究 + 写作 + 事实核查 + 内链 + 发布 | 未公开 | 仅博客，不建站 |
| **ContentClaw** (NPM) | 开源通用内容引擎，从竞品 sitemap 分析 → 9 种内容类型 → 5000+ 页模板模式 | 开源免费 | 仅内容，不建站不部署 |
| **SEOAuto.ai** (WP 插件) | WordPress 批量 AI SEO 页面生成器，3 步向导 | 免费/付费 | WordPress 限定，无策略层 |
| **SEBot** | 全自主 AI SEO Agent：关键词研究 → 内容 → 内链 → 发布，"set and forget" | $19-499/月 | 仅博客，不建站。50+ 语言，9 个 CMS 集成 |

> **关键洞察**：这些"新兴端到端"产品全部停留在 **内容层**——它们可以写文章、发布文章，但**没有一个能建站**。它们依赖 WordPress/Webflow/Shopify 等外部平台来承载内容。这验证了我们的判断：**建站能力本身就是护城河**。

### C.2 6 大结构性缺口（数据验证版）

经过对 22+ 竞品的逐一分析，识别出 6 个无人填补的系统性缺口：

#### 缺口 1：市场研究 → 网站生成 → 部署 → 监控的断层

- **研究工具**（MarketMuse $149+, Frase $45, Surfer $99）→ 告诉你 WHAT 要写
- **内容工具**（Byword $99, SEBot $19, SEOmatic $149）→ 写内容
- **建站工具**（10Web $10, Framer $10, Wix $17, Durable $15）→ 建网站
- **监控工具**（SEMrush $165, Ahrefs $129, Surfer AI Tracker 附加）→ 追踪表现

**没有任何产品连接这四层。** 用户必须手动在 4-6 个工具之间传递数据。每月总花费 $300-800。

#### 缺口 2：竞品网站"拆解"作为建站输入

ContentClaw（开源 NPM）最接近竞品 sitemap 分析，但只生成内容。没有工具：
- 抓取竞品 URL 结构并复制其信息架构
- 分析竞品 Schema 标记并自动实施
- 提取竞品内容深度模式（字数、标题层级、媒体密度）
- 用竞品情报生成**网站结构**（而不仅是内容）

#### 缺口 3："Programmatic SEO 网站"作为产品品类

Byword 生成文章。SEOmatic 从数据生成页面。SEBot 生成博客文章。但**没有人从一个关键词生成一个完整的 Programmatic SEO 网站**——包含正确的 URL 结构、内链架构、Schema 标记、内容层级和模板系统。

#### 缺口 4：Astro 静态站点用于 SEO

大多数 AI 建站器使用：
- WordPress（10Web — 慢、臃肿）
- Wix（封闭生态，SEO 弱）
- Framer（vendor lock-in）
- React/Next.js（Bolt, Lovable, v0 — 对内容站过度）

**没有人生成 Astro 静态站点**，而 Astro 是：
- 比 WordPress 快 3-10 倍
- 内容型 SEO 站点的完美选择
- 几乎零托管成本（CDN 上的静态文件）
- Core Web Vitals 天生高分
- 完全可导出，无锁定

#### 缺口 5：AI/LLM 可见度（GEO）作为默认功能

Surfer 将 "AI Tracker" 作为付费附加。RankSpot 和 Keytomic 提到 LLM 优化。但**没有平台将 LLM/GEO 优化作为网站生成 Pipeline 的核心部分**——为 AI 引用生成结构化数据、构建 LLM 偏好的内容格式、追踪 ChatGPT/Perplexity/Gemini/Claude 中的可见度作为内置功能。

#### 缺口 6：持续监控 + 自动更新

SEOBot 发布后不管。Byword 批量生成。MarketMuse 告诉你需要修复什么。但没有人：
- 持续监控排名和流量
- 检测内容衰减
- 自动重新生成或更新表现不佳的页面
- 提醒竞品变化
- 管理从创建到退役的完整内容生命周期

### C.3 数据成本精确估算

| 数据层 | 最佳来源 | 成本（MVP 规模） | 成本（规模化 1000+ 站） |
|--------|---------|----------------|---------------------|
| SERP 数据 | DataForSEO | $60-200/月（100K 请求） | $500-2,000/月 |
| 关键词数据 | DataForSEO Labs API | ~$100/月（1M 关键词） | $500-2,000/月 |
| 竞品爬取 | ScrapingBee | $99-249/月（1-3M credits） | $599-2,399/月 |
| 反向链接数据 | DataForSEO Backlinks API | $100/月 minimum | $500-2,000/月 |
| LLM 可见度追踪 | DataForSEO LLM Mentions API | $100/月 minimum | $500-1,500/月 |
| AI 内容生成 | Claude API + DeepSeek | $200-500/月 | $2,000-5,000/月 |
| 部署基础设施 | Cloudflare Pages / Vercel | $0-20/月 | $0-200/月 |
| **总计** | | **$800-1,500/月** | **$5,000-15,000/月** |

> 最大成本优化机会：使用 DataForSEO 作为主力数据源（vs SEMrush/Ahrefs 会 5-10x 成本）。

### C.4 竞品定位对比矩阵

| 维度 | 最近竞品 | 我们的优势 |
|------|---------|-----------|
| 市场研究 | MarketMuse ($149+/月) | 全自动，无需手动设置，更便宜 |
| 关键词研究 | DataForSEO Labs ($0.01/任务) | 集成到工作流中，非独立工具 |
| 竞品分析 | Frase/Surfer ($45-99/月) | URL 级别的自动拆解，不仅是内容简报 |
| 内容策略 | MarketMuse ($249+/月) | 从竞品数据自动生成，非手动 |
| Programmatic 页面生成 | SEOmatic ($149/月) | 完整网站而非仅页面；含 IA + Schema + 内链 |
| 内容生成 | Byword ($99/月) | 与网站结构集成，非孤立文章 |
| 网站建设 | 10Web/Framer ($10-30/月) | Astro 静态站（更快、可导出、无锁定） |
| 部署 | Vercel/Netlify ($0-200/月) | Pipeline 自动化的一部分 |
| 监控 | Surfer AI Tracker（附加） | 内置，含自动重新生成触发器 |
| AI/LLM 优化 | RankSpot, Keytomic | 核心功能，非事后附加 |

### C.5 关于 Base44 被收购的最新信息

Base44 于 **2025 年 6 月被 Wix 以约 $8,000 万美元收购**。这验证了"AI 建站"赛道正在快速整合。但也说明 Base44 团队看到了天花板——纯 AI 应用生成器的独立增长空间有限，需要借助 Wix 的分发渠道。这对我们的启示：**不要做另一个 AI App Builder，要做 SEO 工厂——这是 Wix/Base44 不会碰的垂直深度。**

### C.6 最终验证

> 两轮独立调研（一轮面向传统 SEO + 建站工具，一轮深入到 22+ 竞品的逐一分析）得出同一个结论：**AI SEO Website Factory 是一个真实存在的蓝海品类。现有的每一个工具都是点解决方案（Point Solution），而我们的产品是一个系统（System）。**

> **可防御性不来自任何单一功能，而来自集成。如果执行到位，这个产品会让上面所有 22+ 工具在这个特定使用场景下变得不必要。**

---

*附录 C 数据来源：第二轮竞品深度调研，2026-05-24。覆盖 Byword、Surfer、Frase、MarketMuse、SEOmatic、SEBot、10Web、Durable、Wix AI、Hostinger、Relume、Framer AI、Base44、Lovable、Bolt.new、Replit、v0.dev、DataForSEO、SEMrush、Ahrefs、SerpAPI、ScrapingBee + 8 个新兴端到端竞品。*

---

## 附录 D：客户自有 VPS 一键部署方案深度分析

> 核心问题：当客户有自己的 VPS（Hetzner $5/月、阿里云、AWS Lightsail 等），如何实现从平台"一键部署"而无需客户手动操作服务器？

### D.1 六种方案对比

| # | 方案 | 客户操作 | 首次部署 | 后续更新 | 安全风险 | 维护成本 |
|---|------|---------|---------|---------|---------|---------|
| **1** | **SSH Key Push** | 粘贴公钥 + 填 IP | < 30 秒 | `rsync` 增量 | 中（平台持有私钥） | 低 |
| **2** | **Dokploy/Coolify API** | 安装 Dokploy（1 条命令） | API 调用部署 | 自动 Webhook | 低（客户掌控） | 中 |
| **3** | **轻量 Agent（推荐）** | 运行 1 条安装命令 | Agent pull + 启动 | Agent 定时 pull | 低（只读，无写入） | 中 |
| **4** | **GitOps (Git Push → Webhook)** | 配置 Deploy Key | Git push | Webhook 触发 | 低 | 中 |
| **5** | **Container Registry + Watchtower** | 安装 Docker | `docker run` | Watchtower 自动拉取 | 低 | 高 |
| **6** | **GitHub Actions Runner** | 安装 Runner | Actions 执行 | Actions 自动 | 中 | 低 |

### D.2 推荐：方案 3（轻量 Agent）作为主力 + 方案 1（SSH Push）作为备选

#### 方案 3 架构：Lightweight Deploy Agent

```
┌────────────────────────────────────────────────────┐
│                 SEOplant 平台                       │
│                                                      │
│  用户点击"Deploy to my VPS"                           │
│       │                                              │
│       ▼                                              │
│  ┌─────────────┐     HTTPS + API Key                 │
│  │ Deploy      │─────────────────────┐               │
│  │ Orchestrator│                     │               │
│  └─────────────┘                     ▼               │
│                          ┌──────────────────┐        │
│                          │  SEOplant Agent  │        │
│                          │  (客户 VPS)        │        │
│                          │  ~5MB Go binary   │        │
│                          │                   │        │
│                          │  监听 :9876       │        │
│                          │  仅响应本地请求    │        │
│                          └──────┬───────────┘        │
│                                 │                    │
│                    ┌────────────┼────────────┐       │
│                    ▼            ▼            ▼       │
│              ┌─────────┐ ┌──────────┐ ┌──────────┐  │
│              │ Nginx   │ │ SSL Cert │ │ 静态文件  │  │
│              │ (Caddy) │ │ (自动)   │ │ /www/site│  │
│              └─────────┘ └──────────┘ └──────────┘  │
└────────────────────────────────────────────────────┘
```

**客户侧操作（仅一次）**：
```bash
curl -fsSL https://rankforge.com/agent/install.sh | bash
# 安装完成后显示 Agent Key，在平台面板填入即完成绑定
```

**Agent 职责**：
- 接收平台下发的部署指令（HTTPS + API Key 认证）
- 拉取构建产物（静态文件 tarball）
- 写入 `/www/{domain}/dist/`
- 管理 Nginx/Caddy 配置（添加新站点、申请 SSL）
- 可选：管理 Docker 服务（Directus CMS、Plausible 分析）
- 上报健康状态和流量数据

**Agent 特点**：
- Go 单二进制，无依赖，~5MB，内存 < 20MB
- 不需要 root（只需 Nginx 目录的写权限和重载权限）
- 只拉取不推送——客户代码和数据不出 VPS
- 开源 + 可审计——客户知道 Agent 做了什么
- 支持 Ubuntu/Debian/CentOS/Alpine
- 内置 Caddy 集成（自动 SSL）或适配已有 Nginx

#### 方案 1 作为备选：SSH Key Push

当客户不想安装 Agent 时（或 Agent 不兼容时），回退到 SSH 模式：

```python
# deployer.py 增强版伪代码
def deploy_to_vps_ssh(
    host: str,
    user: str = "root",
    port: int = 22,
    ssh_key_path: str = "~/.ssh/rankforge_deploy",
    domain: str,
    project_dir: Path,
):
    """SSH Key-based one-click deploy."""
    client = paramiko.SSHClient()
    client.connect(host, port, user, key_filename=ssh_key_path)

    # 1. 检查服务器环境
    check_cmd = "which nginx && which certbot && echo 'OK'"
    result = client.exec_command(check_cmd)

    # 2. 如需要，初始化服务器（安装 Nginx + Certbot + 创建目录）
    if "OK" not in result:
        bootstrap_server(client)  # apt install nginx certbot

    # 3. 上传静态文件
    with client.open_sftp() as sftp:
        upload_dir(sftp, project_dir / "dist", f"/www/wwwroot/{domain}/dist")

    # 4. 上传 Nginx 配置
    nginx_conf = generate_nginx_config(domain, ssl=True)
    client.exec_command(f"cat > /etc/nginx/sites-available/{domain} <<'EOF'\n{nginx_conf}\nEOF")

    # 5. 启用站点 + SSL
    client.exec_command(f"ln -sf /etc/nginx/sites-available/{domain} /etc/nginx/sites-enabled/")
    client.exec_command(f"certbot --nginx -d {domain} -d www.{domain} --non-interactive")
    client.exec_command("nginx -t && nginx -s reload")
```

### D.3 部署流程对比：Astro 静态站 vs 带 CMS 的站点

| 站点类型 | 最适合方案 | 原因 |
|---------|-----------|------|
| **纯静态 Astro 站**（最常见） | Agent 拉取 tarball → 解压 → Nginx 重载 | 零运行时依赖，CDN 级别性能 |
| **Astro + Directus CMS** | Agent + Docker Compose | CMS 需要持久化数据库和 API 服务 |
| **Astro + Plausible 分析** | Agent + Docker | 分析服务需要运行时 |
| **WordPress（如果用户选择）** | Dokploy 部署 | WordPress 的复杂运行时适合容器编排 |

### D.4 与竞品的差异化

| 竞品 | 部署方式 | 我们的差异化 |
|------|---------|------------|
| **10Web** | 强制使用自家 Google Cloud Hosting | 我们支持**任何 VPS**，用户拥有服务器 |
| **Durable** | 锁定在 Cloudflare CDN | 用户**自选 VPS 提供商**，不限流量 |
| **Wix** | 锁定在 Wix 服务器 | **完整代码导出**，随时迁移 |
| **Byword** | 只发布文章到 CMS | 我们**连服务器一起管** |
| **Vercel/Netlify** | 平台锁定，付费墙 | **不抽成**，用户用自己服务器 |

### D.5 后续部署（Agent 模式下）

一旦 Agent 绑定，后续操作全部在平台面板一键完成：

| 操作 | Agent 执行 |
|------|-----------|
| **内容更新** | 平台触发 → Agent pull 新内容 → 重建静态页面 → 原子替换 dist/ |
| **新增域名** | Caddy/Nginx 自动配置 → 自动申请 SSL |
| **添加 CMS** | `docker compose up -d directus`，自动配置反向代理 |
| **添加分析** | `docker compose up -d plausible`，注入分析脚本 |
| **SSL 续期** | Caddy 自动 / Certbot cron |
| **备份** | Agent 定时打包 `/www` 和数据库 → 上传 S3/rsync |
| **健康检查** | Agent 每 60s 上报 CPU/内存/磁盘/HTTP 状态码 |

### D.6 实施优先级

| 阶段 | 交付 | 说明 |
|------|------|------|
| **Phase 2（当前）** | SSH Push + Manual Script | 平台生成一键脚本，用户复制到服务器执行 |
| **Phase 3** | Agent v1（Go 二进制） | 支持 Ubuntu/Debian，覆盖 80% 用户 |
| **Phase 4** | Agent v2 + Dashboard | 完整 VPS 管理面板，多站点管理、自动备份 |
| **Phase 5** | 多 VPS 编排 | 一个账号管理多台 VPS，负载均衡、故障转移 |

---

> **核心结论：不做 Vercel/Netlify（锁定用户、平台抽成），而是做 "BYOV" (Bring Your Own VPS)。Agent 模式让非技术用户也能在自己的服务器上一键部署，同时保留完全的数据主权和代码所有权。这是与 10Web/Durable/Wix 的本质差异化。**

---

## 附录 E：开源边界策略——吸引客户 vs 防御抄袭

> 核心问题：如果持续在 GitHub 开源，开源什么能吸引客户？开多少不会让竞品一夜之间复制走？

### E.1 框架：Open-Core 三层模型

参考 Supabase、PostHog、GitLab、Cal.com 的成熟实践：

```
┌─────────────────────────────────────────────────────────┐
│                PROPRIETARY (SaaS Only)                   │
│  - AI Agent 编排 (LangGraph workflows)                   │
│  - 真实 SEO 数据管道 (DataForSEO + 缓存 + 成本优化)       │
│  - 内容质量评分算法 (为什么这篇能排，那篇不能)             │
│  - 竞品情报数据库 (每个分析的网站都是我们的数据飞轮)       │
│  - SaaS 平台 (Dashboard, 计费, 多租户, API, Rate Limiting)│
│  - 自主监控 Agent (排名追踪 → 内容衰减 → 自动重新生成)    │
├─────────────────────────────────────────────────────────┤
│           SOURCE-AVAILABLE (BSL / Elastic v2)             │
│  - Programmatic SEO 模板引擎                              │
│  - AI 内容生成 Pipeline (质量闸门, 事实核查, E-E-A-T 检查) │
│  - 关键词集群 & 难度评分 (含真实数据源的版本)              │
├─────────────────────────────────────────────────────────┤
│               OPEN SOURCE (MIT / Apache 2.0)              │
│  - CLI 工具链 (全部 5 个 Python 模块)                     │
│  - Astro 模板 & 组件 (基础版)                              │
│  - DESIGN.md 标准 & 解析器                                 │
│  - 部署 Agent (Go 二进制)                                  │
│  - Nginx/Caddy 配置生成                                    │
│  - Schema 标记模板                                          │
│  - 文档 & 示例 & Tutorial                                   │
└─────────────────────────────────────────────────────────┘
```

### E.2 逐模块决策

| 模块 | 当前代码 | 开源程度 | 许可证 | 理由 |
|------|---------|---------|--------|------|
| **competitor_intel.py** | Jina Search + 爬虫 + SEO 分析 | ✅ MIT 全开源 | MIT | 爬虫代码是 commodity——竞品可以自己写。价值不在爬虫，在我们积累的竞品数据库 |
| **seo_engine.py** | 启发式关键词扩展、聚类、难度评估、内容日历 | ✅ MIT（当前版本）⚠️ BSL（接入真实数据后） | MIT → BSL | 当前纯启发式没有保护价值。接入 DataForSEO 后的数据管道版本用 BSL 保护 |
| **design_intel.py** | 设计 Token 提取、DESIGN.md 生成、设计融合 | ✅ MIT 全开源 | MIT | 设计提取是 commodity。护城河在我们的设计库规模（社区贡献 + 我们分析的站点） |
| **site_builder.py** | Astro 脚手架、CSS 生成、组件生成 | ✅ MIT 全开源 | MIT | 脚手架是 commodity。模板质量才是差异——基础模板开源，Premium 模板付费 |
| **deployer.py** | VPS 部署、Nginx/Caddy 配置、Server Bootstrap | ✅ MIT 全开源 | MIT | **信任工具**——客户必须能审计部署 Agent 的代码，否则不会装到自己的服务器上 |
| **Programmatic SEO 引擎** | 未开发 | ⚠️ BSL | BSL | 这是核心差异化——模板 + 数据集 → 5000 页的能力。BSL 允许自用，禁止商业竞争 |
| **AI Agent 编排** | 未开发 | 🔒 Proprietary | - | 多 Agent 协调逻辑——这是我们 PK 单一功能的竞品的核心 |
| **内容质量评分** | 未开发 | 🔒 Proprietary | - | 核心 IP——"什么内容能排名"的判断模型 |
| **SaaS Dashboard** | 未开发 | 🔒 Proprietary | - | 标准 SaaS，无需开源 |
| **监控 Agent** | 未开发 | 🔒 Proprietary | - | 排名追踪 + 内容衰减检测 + 自动更新——"持续增长"闭环 |

### E.3 为什么这个策略能同时"吸引客户"和"防御抄袭"

#### 开源部分的作用（吸引客户）

| 开源内容 | 吸引的客户 | 获客路径 |
|---------|-----------|---------|
| **CLI 工具链** | 技术型 SEO、独立开发者 | "我可以先自己跑通，验证效果好再付费 SaaS" |
| **部署 Agent 源码** | 所有 VPS 用户 | "我需要看到 Agent 做了什么才敢装到服务器上" |
| **Astro 模板** | 前端开发者 | "模板不错，但我不想要手动做 SEO 策略——试试 SaaS 版" |
| **DESIGN.md 标准** | 设计师、设计工具 | "这个格式好用，我基于它做了工具——你们的平台支持吗？" |
| **文档/教程** | 所有人 | SEO 教程在 Google 有搜索量——教程本身是获客内容 |

#### 闭源部分不伤及获客

| 闭源内容 | 为什么客户不介意 |
|---------|---------------|
| **SEO 数据管道** | 客户关心"数据准不准"，不关心代码怎么调 API |
| **AI Agent 编排** | 客户关心"生成的网站能不能排名"，不关心内部 workflow |
| **内容质量算法** | 客户关心"内容好不好"，黑盒结果可以接受 |
| **SaaS Dashboard** | 客户期待的就是托管服务 |

#### 竞争者无法仅靠开源代码复制我们

假设一个竞品（比如 Byword 或 10Web）把我们的 GitHub 仓库 fork 走：

| 他们有 | 他们没有 |
|--------|---------|
| ✅ 关键词扩展模板代码 | ❌ 真实搜索量/KD/CPC 数据（需要 DataForSEO 合同 + 缓存层） |
| ✅ 竞品爬虫代码 | ❌ 已分析的数千个 niche 的竞品数据库 |
| ✅ Astro 脚手架代码 | ❌ 经过训练的"什么结构对 SEO 最优"的规则引擎 |
| ✅ 部署脚本 | ❌ 持续监控 + 内容衰减检测 + 自动重新生成 |
| ✅ 设计 Token 提取器 | ❌ 已经分析过的 10,000+ 设计参考库 |
| ✅ 基础 SEO 报告模板 | ❌ 多 Agent 协调逻辑（每个 niche 调用的 Agent 顺序不同） |
| ✅ pSEO 模板引擎（如果开源） | ❌ AI 内容质量评分模型（哪篇该改、哪篇该删） |

> **核心防御逻辑：代码是 commodity，数据和运行中的 AI 系统才是护城河。我们开源的是"给你看我们怎么做的"，闭源的是"帮你做好的结果"。**

### E.4 参考标杆

| 公司 | 开源内容 | 闭源内容 | 融资/估值 | 对我们的启示 |
|------|---------|---------|---------|------------|
| **Supabase** | 数据库核心 (MIT)、SDK、CLI | Dashboard、Auth 高级功能、实时数据库托管 | $196M, $2B | 开源 Firebase 替代品——我们开源"SEO 平台替代品" |
| **PostHog** | 产品分析核心 (MIT)、SDK | 特征开关、A/B 测试高级版、Session Replay | $92M | 开源 Mixpanel ——开源建立信任（数据不出自己的服务器） |
| **GitLab** | CE 版 (MIT) — 全功能 | EE 版 — 企业功能（LDAP、审计日志） | IPO $11B | 开源自托管 → 企业付费 SaaS |
| **Cal.com** | 调度核心 (MIT) | Teams、SSO、White-label、Analytics | $32M | 开源 Calendly ——我们将是"开源 Programmatic SEO 平台" |
| **Activepieces** | 自动化核心 (MIT) | Enterprise features | — | 开源 Zapier/Make ——同理，开源 SEO 工作流 |
| **Mattermost** | Team Edition (MIT) | Enterprise (SSO, compliance) | $200M+ | 开源 Slack ——社区版完整可用，企业版溢价 |

### E.5 关于 "open-seo" 和 "seomachine" 的出现

调研中发现了两个直接相关的开源项目：

- **[open-seo](https://github.com/every-app/open-seo)**（v0.0.5, 2026-04）：开源 SEMrush/Ahrefs 替代品，自己 host，按 API 使用量付费。验证了 SEO 领域的"开源核心 + API 付费"模式有需求。
- **[seomachine](https://github.com/TheCraigHewitt/seomachine)**（6,900+ GitHub Stars, 945 Forks）：Claude Code 驱动的 SEO CLI 工具，7 个 AI Agent。我们的 tool-registry.md 已经引用了它。

> 这些项目的存在**不是威胁，而是验证**——市场正在用 GitHub Stars 投票：人们想要开源的 SEO 工具。但他们都停留在"内容/分析层"，没有进入"工厂/建站层"。我们是第一个做端到端的。

### E.6 执行时间线

| 阶段 | 开源动作 | 目的 |
|------|---------|------|
| **现在** | GitHub 仓库公开，MIT 协议，README 写清楚"这个 CLI 能做什么" | 建立基础存在，吸引早期关注 |
| **Phase 1-2** | 添加 CONTRIBUTING.md、Roadmap、5 个模块的独立文档 | 降低贡献门槛，鼓励社区参与 |
| **Phase 3** | 发布 pSEO 模板引擎（BSL 协议），发布 5 个完整示例站点 | 展示"工厂"能力，BSL 防止直接商业竞争 |
| **Phase 4** | 开放设计库的社区提交通道，发布 Deploy Agent v1（MIT） | 社区贡献数据飞轮，Agent 开源建立信任 |
| **Phase 5** | 发布 Hosted API（付费），Dashboard 公开注册 | 货币化——"你可以自己跑 CLI，也可以用我们的 SaaS" |

### E.7 一句话总结

> **开源"怎么做 SEO 工厂"的全部代码 → 吸引技术型用户验证 → 他们变成口碑传播者 → 非技术客户看到效果后付费 SaaS → 竞品能抄代码但抄不了数据和 AI 系统 → 我们跑得比他们快。**

---

*附录 E 基于：Supabase/PostHog/GitLab/Cal.com 的 open-core 模式分析 + open-seo/seomachine/OpenOkara 的直接竞品分析（2026-05）。*

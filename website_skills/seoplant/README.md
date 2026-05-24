# SEOplant

> **输入一个关键词，AI 自动完成竞品分析 → SEO 战略 → 网站生成 → 部署上线。**
>
> From keyword to ranking website. Automatically.
>
> **[seoplant.io](https://seoplant.io)** — AI Programmatic SEO Platform

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Python 3.10+](https://img.shields.io/badge/Python-3.10%2B-blue.svg)](https://python.org)

---

## What is SEOplant?

SEOplant is an **open-source AI Programmatic SEO Platform** — not just another AI website builder.

Existing tools either:
- **Analyze SEO** but can't build sites (Surfer, Frase, Byword)
- **Build sites** but don't understand SEO (10Web, Durable, Wix AI)

SEOplant does both. Give it a keyword or niche:

| Step | What Happens | Module |
|------|-------------|--------|
| 1. **Market Intel** | Scans competitors, analyzes SERP landscape, estimates traffic potential | `competitor_intel.py` |
| 2. **SEO Strategy** | Generates topic clusters, content calendar, keyword difficulty matrix | `seo_engine.py` |
| 3. **Design System** | Extracts design tokens from winning sites, fuses into DESIGN.md | `design_intel.py` |
| 4. **Site Factory** | Scaffolds Astro static site with Schema, llms.txt, i18n routing | `site_builder.py` |
| 5. **Deploy** | One-click push to your own VPS (SSH) or Vercel/Cloudflare | `deployer.py` |

---

## Quick Start

```bash
git clone https://github.com/RichardDu1/seoplant.git
cd seoplant
pip install requests beautifulsoup4

# Analyze competitors
python scripts/competitor_intel.py report "best hiking boots"

# Generate SEO plan
python scripts/seo_engine.py plan "scottish highlands travel" --type travel

# Scaffold Astro site
python scripts/site_builder.py scaffold ./my-site --name "My Site" --keyword "scottish highlands travel"

# Deploy to your VPS
python scripts/deployer.py deploy ./my-site mysite.com --host 12.34.56.78 --caddy
```

---

## Architecture

```
Keyword / Niche
      │
      ▼
┌──────────────────┐
│ competitor_intel │  ← Jina Search + SERP analysis
│ Phase 1          │
└────────┬─────────┘
         │ competitor report
         ▼
┌──────────────────┐
│ seo_engine       │  ← Keyword expansion, clustering, content calendar
│ Phase 2          │
└────────┬─────────┘
         │ SEO blueprint
         ▼
┌──────────────────┐
│ design_intel     │  ← Design token extraction, DESIGN.md generation
│ Phase 3          │
└────────┬─────────┘
         │ design system
         ▼
┌──────────────────┐
│ site_builder     │  ← Astro scaffolding, Schema markup, i18n routing
│ Phase 4          │
└────────┬─────────┘
         │ complete Astro project
         ▼
┌──────────────────┐
│ deployer         │  ← VPS SSH push / Vercel / Cloudflare
│ Phase 5          │
└────────┬─────────┘
         │
         ▼
   🚀 Live Site
```

---

## Why Open Source?

- **No vendor lock-in** — code is yours, site is yours, server is yours
- **Auditable** — see what the deploy agent does before installing it on your server
- **BYOV (Bring Your Own VPS)** — use your $5/month VPS, not overpriced hosting
- **Extensible** — fork, modify, integrate into your agency workflow

The cloud platform at **[seoplant.io](https://seoplant.io)** adds: real SEO data (DataForSEO), AI agent orchestration, programmatic SEO at scale (5000+ pages), autonomous rank monitoring.

---

## Requirements

- Python 3.10+
- `pip install requests beautifulsoup4`
- Astro (for site generation): `npm create astro@latest`
- SSH access (for VPS deployment)
- Optional: `pip install paramiko` (richer SSH error handling)

---

## File Structure

```
seoplant/
├── README.md
├── LICENSE (MIT)
├── scripts/
│   ├── competitor_intel.py     # Phase 1: competitor research + SEO analysis
│   ├── seo_engine.py           # Phase 2: keyword planning + GEO config
│   ├── design_intel.py         # Phase 3: design token extraction
│   ├── site_builder.py         # Phase 4: Astro project scaffolding
│   └── deployer.py             # Phase 5: VPS/Vercel/Cloudflare deployment
├── references/
│   └── tool-registry.md        # Tool capability registry
└── design-library/
    └── index.json              # Design reference library
```

---

## Roadmap

| Phase | Status | What |
|-------|--------|------|
| **1. CLI Tools** | Done | All 5 modules work standalone |
| **2. DataForSEO Integration** | In Progress | Real search volume, KD, CPC data |
| **3. SaaS Dashboard** | Planned | Multi-site management, credits, API |
| **4. Programmatic SEO Engine** | Planned | Template + dataset → 5000+ pages |
| **5. Autonomous Agent** | Planned | Rank monitoring → content refresh → auto-expand |

---

## License

MIT — see [LICENSE](LICENSE)

The deploy agent is also MIT (auditable by design — you should know what runs on your server).

---

**[seoplant.io](https://seoplant.io)** — From keyword to ranking website.

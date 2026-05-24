---
name: website-generator
description: |
  End-to-end website generation workflow. Takes a keyword/description and produces
  a production-ready static website with SEO/GEO optimization, competitive design,
  and deployment configuration. Orchestrates 5 phases: Competitor Intelligence →
  SEO/GEO Planning → Design Intelligence → Site Generation → Deployment.
version: "3.0"
author: RichardDu1
tags: [website, seo, geo, design, astro, i18n, deployment]
---

# Website Generator Skill

## Overview

This skill takes a **keyword or description** (e.g. "苏格兰高地旅游网站") and orchestrates a complete pipeline to generate a production-ready static website. It works across Claude Code, Codex, Antigravity, and Hermes.

## Universal Compatibility

To ensure identical results across all platforms, this skill follows three principles:
1. **File-based state** — all intermediate artifacts are saved as `.md`, `.yaml`, `.json`. Any agent can resume from any phase.
2. **Fallback chains** — every tool has a fallback (Tavily → DuckDuckGo, Pexels → Pixabay, Stagehand → Playwright).
3. **Platform detection** — agents detect their available tools and adapt (see [Notes for Agent Compatibility](#notes-for-agent-compatibility)).

| Platform | Search | Crawl | Browser | Image Source | Deploy |
|----------|--------|-------|---------|-------------|--------|
| **Claude Code** | WebSearch | WebFetch (Jina) | Playwright MCP | Pexels via browser | paramiko SSH |
| **Hermes** | Tavily/DuckDuckGo | Jina Reader | mcp-chrome | Pexels/Tavily | paramiko SSH |
| **Codex/Antigravity** | native search | native fetch | native browser | Pexels if browser; Pixabay fallback | paramiko SSH |

## Prerequisites

Before starting, ensure the following tools are available. The skill will verify and guide installation as needed.

### Required Tools
- **Node.js** ≥ 18 (for Astro, npx tools)
- **Python** ≥ 3.10 (for scripts)
- **Git** (for cloning templates)

### Required Agent Skills (install if missing)
- `geo-seo-claude` — GEO + traditional SEO analysis
- `claude-seo` — E-E-A-T content quality + Schema markup
- `seomachine` — keyword research → content → publish pipeline

### Optional Integrations
- Tavily API key (for search; falls back to DuckDuckGo)
- Jina Reader (free, no key needed: `r.jina.ai`)
- Directus (self-hosted CMS, Docker-based)

### Design Quality Tools (install for Phase 4)
- **Impeccable** — UI polish, audit, distill (20+ design commands)
  Install: `npx skills add pbakaus/impeccable`
  Repo: [pbakaus/impeccable](https://github.com/pbakaus/impeccable)
- **Better Icons** — 200K+ icons from 150+ collections (MCP server)
  Install: `npx better-icons setup`
  Repo: [better-auth/better-icons](https://github.com/better-auth/better-icons)
- **Taste Skill** (optional) — Premium aesthetic enforcement, anti-slop rules
  Install: `npx skills add Leonxlnx/taste-skill`
  Variants: `soft-skill` (luxury) | `minimalist-skill` | `brutalist-skill`
  Repo: [Leonxlnx/taste-skill](https://github.com/Leonxlnx/taste-skill)

---

## How To Use

### Quick Start
```
User: 帮我生成一个关于 [关键词] 的 [类型] 网站
```

### Full Input Format
```yaml
keyword: "scottish highlands travel"
site_type: "travel blog"           # travel | ecommerce | saas | portfolio | docs
languages: ["en", "zh"]            # ISO 639-1 codes
target_markets: ["UK", "CN"]       # ISO 3166-1 alpha-2
modules:                           # optional
  cms: true                        # Directus
  map: true                        # Leaflet + OpenStreetMap
  ecommerce: false                 # Medusa
  analytics: true                  # Plausible / Tianji
deploy:
  type: "vps"                      # vps | vercel | netlify
  panel: "baota"                   # baota | dokploy | manual
```

---

## Execution Pipeline

The skill runs 5 phases sequentially. Each phase produces artifacts that feed the next. The agent MUST complete each phase and get user confirmation before proceeding.

### Phase 0: Requirement Clarification

**Goal**: Understand exactly what the user wants.

**Steps**:
1. Parse the user's keyword and intent
2. Ask clarifying questions if needed:
   - What type of website? (travel, ecommerce, SaaS, portfolio, docs)
   - Target languages and markets?
   - Need CMS backend? Map integration? E-commerce?
   - Deployment preference? (own VPS with Baota, or cloud platforms)
3. Set defaults for anything unspecified:
   - `site_type`: infer from keyword
   - `languages`: `["en"]`
   - `modules.cms`: `true` (Directus)
   - `modules.analytics`: `true` (Plausible)
   - `deploy.type`: `vps`

**Output**: `project-brief.yaml` in project root

---

### Phase 1: Competitor Intelligence

**Goal**: Understand the competitive landscape — websites, social media, and advertising.

**Script**: `scripts/competitor_intel.py`

**Steps**:

#### 1.1 Search Competitors
```
Search for 10-15 competitors using:
  - Tavily search: "{keyword} best websites"
  - DuckDuckGo: "{keyword} top sites 2025"
  - Google: "site:awwwards.com {keyword}" (design quality)
Filter to top 5 most relevant competitors.
```

#### 1.2 Crawl & Analyze Websites
```
For each competitor URL:
  1. Fetch with Jina Reader: https://r.jina.ai/{url}
     → Extract: page structure, headings, word count, internal links
  2. Check tech stack:
     → Use Wappalyzer patterns or BuiltWith-style headers analysis
  3. Analyze SEO signals:
     → Title tags, meta descriptions, H1 structure
     → Schema markup presence
     → Page load indicators (resource count, JS size)
  4. Capture screenshot (if browser available):
     → Use Stagehand or Playwright for visual reference
```

#### 1.3 Analyze Social Media & Ads
```
For each competitor:
  1. Search social presence:
     → "{competitor name} site:instagram.com OR site:twitter.com OR site:facebook.com"
  2. Check Facebook Ad Library:
     → https://www.facebook.com/ads/library/?q={competitor}
     → Note: ad creative themes, CTAs, target demographics
  3. Summarize social strategy:
     → Posting frequency, content types, engagement signals
```

#### 1.4 Generate Report
```
Output: competitor-report.md containing:
  - SWOT analysis for each competitor
  - Gap analysis: what nobody is doing well
  - Opportunity matrix: where we can differentiate
  - Design quality ranking (1-10)
  - Content quality ranking (1-10)
  - SEO strength ranking (1-10)
```

**Artifact**: `competitor-report.md`

**User Checkpoint**: Present report, ask for feedback before proceeding.

---

### Phase 2: SEO/GEO Planning

**Goal**: Build a comprehensive keyword strategy and content plan.

**Script**: `scripts/seo_engine.py`

**Depends on**: Phase 1 competitor report

**Steps**:

#### 2.1 Keyword Research
```
Use geo-seo-claude skill methodology:
  1. Seed keywords from user input + competitor analysis
  2. Expand with long-tail variations:
     → "{keyword} + how/what/where/when/best/guide/tips"
     → "{keyword} + {location}" for geo-targeting
     → "{keyword} + {year}" for freshness
  3. Cluster into Topic Pillars (5-8 pillars)
  4. For each cluster, identify:
     → Primary keyword (highest volume)
     → Supporting keywords (5-15 per cluster)
     → Question keywords (People Also Ask format)
```

#### 2.2 Difficulty Assessment
```
For each keyword cluster:
  1. Estimate difficulty: Low / Medium / High
     → Based on competitor analysis from Phase 1
     → SERP feature analysis (featured snippets, knowledge panels)
  2. Assign priority: ★★★ (quick win) / ★★ (medium term) / ★ (long term)
  3. Map to content types:
     → Pillar page (2000+ words)
     → Blog post (1000-1500 words)
     → FAQ page
     → Tool/calculator page
```

#### 2.3 Content Calendar
```
Generate a 6-month content plan:
  Month 1-2: Foundation
    → 5 pillar pages + 10 supporting posts (low difficulty ★★★)
  Month 3-4: Growth
    → 15 supporting posts (medium difficulty ★★)
    → Link building targets
  Month 5-6: Authority
    → 10 posts targeting harder keywords (★)
    → GEO optimization for AI search engines
```

#### 2.4 GEO Strategy
```
Apply geo-seo-claude principles:
  1. Generate llms.txt for AI search engine optimization
  2. Create Schema.org markup templates:
     → Organization, WebSite, Article, FAQPage, BreadcrumbList
     → Site-type specific: TouristAttraction, Product, LocalBusiness
  3. Structured data for key entities
  4. Citation optimization patterns
```

#### 2.5 Multi-language Keyword Strategy
```
CRITICAL: Do NOT just translate keywords. Build independent keyword systems.
For each target language:
  1. Research native search behavior
  2. Find language-specific long-tail opportunities
  3. Identify cultural differences in search intent
  4. Map cross-language content relationships (hreflang)
```

#### 2.6 Automated Execution Strategy (Hermes)
```
Since you are Hermes (an autonomous agent), set up continuous SEO operations:
  1. Content Auto-Pilot: Use `/schedule` to run weekly loops where you pull a keyword from the calendar, research it, write an E-E-A-T article, and POST it directly to the Directus CMS API.
  2. Link Building: Use web tools to scrape competitor backlinks or unlinked mentions, then draft outreach emails to `outreach/`.
  3. Social Promotion: After publishing, use `marketingskills` to auto-generate Twitter/X threads and LinkedIn posts for distribution.
```

**Artifact**: `seo-plan.md` + `content-calendar.md`

**User Checkpoint**: Present SEO plan, get approval before design phase.

---

### Phase 3: Design Intelligence

**Goal**: Find, extract, and fuse design inspiration from real websites into a project DESIGN.md.

**Script**: `scripts/design_intel.py`

**Depends on**: Phase 1 (competitor URLs) + Phase 2 (brand positioning from SEO)

> [!IMPORTANT]
> This is NOT "pick from 3 templates". This is an intelligent design search engine.
> **Strategy**: Fusion-first (融合优先). Extract design tokens from multiple real websites,
> then fuse into an original design. This avoids plagiarism concerns while maintaining quality.
> Direct cloning is available as a secondary option via ai-website-cloner-template.

**Steps**:

#### 3.1 Search Design Inspiration
```
Search for excellent websites in the same niche:
  1. Design galleries (search in order):
      → Awwwards: "https://www.awwwards.com/websites/{category}/"
     → PageFlows: "https://pageflows.com" (real user flow screenshots)
     → Godly: "https://godly.website" (curated modern web design)
     → Mobbin: "https://mobbin.com" (mobile + web UI patterns)
     → Land-book: "https://land-book.com" (landing pages, has color filters)
     → DesignEverywhere: "https://designeverywhere.co" (global design showcase)
     → SiteInspire: filtered by industry
     → Lapa Ninja: landing page designs
  2. Direct competitors (from Phase 1)
  3. Adjacent industries with transferable design patterns
  4. Google: "best {site_type} website design 2025"

Collect 10-15 candidate URLs.
```

#### 3.2 Extract Design Systems
```
For each candidate URL, extract design tokens:

Method A — CLI extraction (preferred):
  $ npx dembrandt {url}
  → Outputs: colors, typography, spacing, component patterns

Method B — Jina Reader + Agent analysis:
  Fetch https://r.jina.ai/{url}
  Then analyze the HTML/CSS to extract:
  → Color palette (primary, secondary, accent, neutral)
  → Typography (font families, scale, weights)
  → Spacing system (base unit, scale)
  → Layout patterns (grid, sections, hero style)
  → Component library (cards, buttons, navigation)

Method C — Screenshot + Vision analysis:
  Capture screenshot with Playwright/Stagehand
  Use vision model to describe design language

Store each as a structured DESIGN.md in design-library/extracted/
```

#### 3.3 Curate & Present
```
From the 10-15 extracted designs:
  1. Score relevance to our niche (1-10)
  2. Score design quality (1-10)
  3. Score technical modernity (1-10)
  4. Select top 5-8 to present to user

Present each with:
  → Website name and URL
  → Screenshot (if available)
  → Key design characteristics (dark/light, minimal/rich, etc.)
  → Color palette preview
  → Typography summary
  → Why it's relevant to our project
```

#### 3.4 User Selection & Fusion (Primary Path ✅)
```
User selects 1-3 designs they like.

AI fusion process (recommended — avoids plagiarism):
  1. Extract common patterns from selected designs
  2. Resolve conflicts (e.g., one uses dark mode, another light)
  3. Apply brand positioning from SEO phase
  4. Generate unified DESIGN.md with:
     → YAML frontmatter (machine-readable tokens)
     → Markdown body (design philosophy + component guide)
     → Tailwind/CSS variable mappings
  5. Result is an ORIGINAL design inspired by real sites, not a copy
```

#### 3.4b Direct Clone (Secondary Path)
```
If user prefers to clone a specific design:
  Use ai-website-cloner-template to clone the selected website.
  Then customize content, colors, and branding.
  NOTE: Only use when user explicitly requests cloning.
```

#### 3.5 Reference awesome-design-md
```
Also check VoltAgent/awesome-design-md for brand-level references:
  → Stripe, Linear, Vercel, etc. design systems
  → Use as quality benchmarks and pattern libraries
  → Merge relevant patterns into our DESIGN.md
```

**Artifact**: `DESIGN.md` (project design system)

**User Checkpoint**: User reviews and approves DESIGN.md.

---

### Phase 4: Site Generation

**Goal**: Generate a complete Astro-based static website from DESIGN.md + SEO plan.

**Script**: `scripts/site_builder.py`

**Depends on**: Phase 2 (SEO plan) + Phase 3 (DESIGN.md)

**Steps**:

#### 4.1 Initialize Project
```
$ npx create-astro@latest ./ --template minimal --install --no-git --typescript strict

Install core dependencies:
  $ npm install @astrojs/sitemap @astrojs/mdx astro-i18n-aut

Install optional dependencies based on modules:
  - Map: $ npm install leaflet
  - Animation: $ npm install animejs
  - CMS: configure Directus REST client
```

#### 4.2 Generate Site Structure
```
Based on SEO plan, create:

src/
├── layouts/
│   ├── BaseLayout.astro          # Global layout with SEO meta
│   └── BlogLayout.astro          # Blog post layout
├── components/
│   ├── Header.astro              # Navigation
│   ├── Footer.astro              # Footer with sitemap links
│   ├── Hero.astro                # Hero section
│   ├── Card.astro                # Content card
│   ├── SEOHead.astro             # Dynamic SEO meta tags
│   ├── SchemaMarkup.astro        # JSON-LD Schema.org
│   └── [module-specific]/        # Map, CMS, etc.
├── pages/
│   ├── index.astro               # Homepage (pillar)
│   ├── [pillar-pages].astro      # From SEO plan pillars
│   ├── blog/
│   │   └── [...slug].astro       # Blog posts from content calendar
│   └── [lang]/                   # i18n routes
│       ├── index.astro
│       └── ...
├── content/
│   └── blog/                     # Markdown content files
├── i18n/
│   ├── en.json                   # English translations
│   └── [lang].json               # Other languages
├── styles/
│   └── global.css                # From DESIGN.md tokens
└── data/
    └── navigation.json           # Site navigation structure

Root files:
├── astro.config.mjs
├── DESIGN.md                     # From Phase 3
├── llms.txt                      # GEO optimization
├── robots.txt
└── public/
    ├── sitemap.xml               # Auto-generated
    └── favicon.svg
```

#### 4.2b Icon Management
```
Use Better Icons MCP to search and sync icons:
  1. Match icon collection to DESIGN.md aesthetic:
     → Minimal design → Lucide / Heroicons
     → Rich design → Phosphor / Tabler
     → Brand icons → Simple Icons
  2. Search icons for each component: navigation, features, footer, social
  3. Sync to project: npx better-icons get {collection}:{name} --size 24
  4. Save as SVG components in src/components/icons/
```

#### 4.3 Apply Design System
```
Convert DESIGN.md tokens to CSS custom properties:
  :root {
    --color-primary: {from DESIGN.md};
    --color-secondary: {from DESIGN.md};
    --font-heading: {from DESIGN.md};
    --font-body: {from DESIGN.md};
    --spacing-base: {from DESIGN.md};
    /* ... full token set */
  }

Import Google Fonts specified in DESIGN.md.
Apply component patterns from DESIGN.md to Astro components.
```

#### 4.4 Inject SEO
```
For every page:
  1. Dynamic <title> and <meta description> from SEO plan
  2. Open Graph tags (og:title, og:description, og:image)
  3. Twitter Card tags
  4. Schema.org JSON-LD (type-appropriate)
  5. hreflang tags for multi-language
  6. Canonical URLs
  7. Breadcrumb navigation
```

#### 4.5 Generate Content
```
For each page in the content calendar (Phase 2):
  1. Generate page content following SEO guidelines:
     → Target keyword in H1, first paragraph, and naturally throughout
     → E-E-A-T signals (author info, sources, dates)
     → Internal linking to related cluster pages
     → FAQ sections (from "People Also Ask" keywords)
  2. Create frontmatter:
     → title, description, keywords, author, date, image
  3. Save as .md files in src/content/blog/
```

#### 4.6 Integrate Optional Modules

**CMS (Directus)**:
```
Generate docker-compose.directus.yml:
  services:
    directus:
      image: directus/directus:latest
      ports: ["8055:8055"]
      environment:
        ADMIN_EMAIL: admin@example.com
        ADMIN_PASSWORD: changeme
        DB_CLIENT: sqlite3
      volumes:
        - ./directus-data:/directus/database
        - ./directus-uploads:/directus/uploads

Generate Astro integration:
  → REST client for fetching Directus content
  → Dynamic routes for CMS-managed pages
```

**Map (Leaflet)**:
```
Generate src/components/Map.astro:
  → Leaflet + OpenStreetMap tiles
  → GeoJSON data layer for locations
  → Responsive container
  → Client-side hydration (client:only="preact")
```

**E-commerce (Medusa)**:
```
Generate docker-compose.medusa.yml
Generate product listing components
Generate cart/checkout flow
```

**Animation (anime.js)**:
```
Generate src/scripts/animations.js:
  → Scroll-triggered animations
  → Hero entrance animations
  → Card hover effects
  → Page transition effects
```

#### 4.7 Build & Validate
```
$ npm run build

Validate:
  1. All pages render without errors
  2. Lighthouse audit (aim for 90+ across all categories)
  3. HTML validation
  4. Schema.org validation (Google Rich Results Test format)
  5. Sitemap.xml is complete
  6. robots.txt is correct
  7. All images have alt text
  8. All links resolve
```

#### 4.8 Design Polish (Impeccable)
```
Run Impeccable design quality passes on all generated pages:

  Pass 1 — /polish
    → Fine-tune spacing, alignment, typography micro-details
    → Fix visual rhythm and whitespace balance
    → Ensure consistent border radii, shadows, padding

  Pass 2 — /audit
    → Catch design inconsistencies across pages
    → Flag accessibility contrast issues
    → Verify heading hierarchy visual weight

  Pass 3 — /distill
    → Remove visual noise and redundant elements
    → Simplify over-designed components
    → Ensure content-first visual hierarchy

Optional — Taste Skill enhancement:
  If site needs premium feel, apply Taste Skill variant:
    → Travel/brand sites: soft-skill (luxury aesthetic)
    → Tech/tool sites: minimalist-skill
    → Portfolio: default taste-skill
  Taste Skill adds: refined whitespace, premium typography,
  spring animations, anti-slop rules (blocks AI-generic patterns)
```

**Artifact**: Complete Astro project in `./`

---

### Phase 5: Deployment

**Goal**: Deploy to production on user's VPS.

**Script**: `scripts/deployer.py`

**Steps**:

#### 5.1 Generate Nginx Config
```
Generate nginx.conf for Baota panel:
  server {
    listen 80;
    server_name {domain};
    root /www/wwwroot/{domain}/dist;
    index index.html;

    # Gzip
    gzip on;
    gzip_types text/css application/javascript application/json image/svg+xml;

    # Cache static assets
    location ~* \.(css|js|jpg|jpeg|png|gif|ico|svg|woff2)$ {
      expires 1y;
      add_header Cache-Control "public, immutable";
    }

    # SPA fallback
    location / {
      try_files $uri $uri/ $uri.html =404;
    }

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    # Baota panel handles SSL natively; do NOT setup SSL inside Docker/Nginx manually here unless requested.
    
    # CMS Reverse Proxy (Directus usually on 8055)
    location /cms/ {
      proxy_pass http://127.0.0.1:8055/;
      proxy_set_header Host $host;
      proxy_set_header X-Real-IP $remote_addr;
    }
  }
```

> [!WARNING]
> **Baota Panel (宝塔) Integration Notes:**
> 1. **Reverse Proxy:** Directus runs on Docker (default port 8055). You must configure Baota's Nginx to reverse proxy `/cms/` (or a subdomain like `cms.domain.com`) to `127.0.0.1:8055`.
> 2. **SSL:** Let Baota's Let's Encrypt manager handle SSL. Do not configure SSL inside `docker-compose.yml`, it will cause conflicts.
> 3. **Static vs SSR:** If Astro builds as static (`dist/`), Baota's standard PHP/HTML site template works perfectly. If using Astro SSR (Node server), Baota must reverse proxy the main domain to Astro's Node port (e.g., 4321).
> 4. **Permissions:** Ensure the static `dist/` files are owned by `www:www` so Baota Nginx can serve them without 403 Forbidden errors.

#### 5.2 Generate Docker Compose (if CMS/services needed)
```
Generate docker-compose.yml combining:
  - Directus (if CMS enabled)
  - Medusa (if ecommerce enabled)
  - Plausible/Tianji (if analytics enabled)
  - Nginx reverse proxy
```

#### 5.3 Generate Deployment Script
```
Generate deploy.sh:
  #!/bin/bash
  npm run build
  rsync -avz --delete dist/ user@server:/www/wwwroot/{domain}/dist/
  ssh user@server "nginx -t && nginx -s reload"
```

#### 5.4 Analytics Setup
```
Generate analytics integration:
  - Plausible: self-hosted script tag
  - Tianji: alternative if already installed
  - Google Search Console: verification file
```

#### 5.5 SEO Monitoring
```
Configure toprank (from your Stars):
  - Track target keywords from SEO plan
  - Weekly ranking reports
  - Alert on significant changes
```

**Artifact**: `deploy/` directory with all config files

---

## File Reference

| File | Purpose |
|------|---------|
| `scripts/competitor_intel.py` | Phase 1 automation helpers |
| `scripts/seo_engine.py` | Phase 2 SEO analysis helpers |
| `scripts/design_intel.py` | Phase 3 design extraction & fusion |
| `scripts/site_builder.py` | Phase 4 project scaffolding helpers |
| `scripts/deployer.py` | Phase 5 deployment config generators |
| `design-library/index.json` | Local design reference index |
| `references/tool-registry.md` | Complete tool capability registry |

---

## Tool Registry (Quick Reference)

### SEO/GEO
| Tool | Function | Install |
|------|----------|---------|
| geo-seo-claude | GEO + SEO analysis | Clone from GitHub |
| claude-seo | E-E-A-T + Schema | Clone from GitHub |
| seomachine | Full SEO pipeline | Clone from GitHub |
| toprank | Rank tracking | Clone from GitHub |
| marketingskills | Marketing strategy | Clone from GitHub |

### Web Crawling (escalation ladder)
| Level | Tool | When to Use |
|-------|------|-------------|
| 1 | `r.jina.ai/{url}` | Simple pages, free, no setup |
| 2 | `spider-rs` / `Crawl4AI` | Batch crawling, high performance |
| 3 | `Scrapling` / `ScrapeGraphAI` | Anti-bot bypass, AI extraction |
| 4 | `CamoFox` / `CloakBrowser` | Stealth browsing needed |
| 5 | `Stagehand` / `Agent-S` | Full browser automation |

### Design
| Tool | Function |
|------|----------|
| `npx dembrandt {url}` | Extract DESIGN.md from URL |
| `awesome-design-md` | Brand design reference library |
| `superdesign` | AI Product Design Agent |
| `ui-ux-pro-max-skill` | AI UI/UX design intelligence |
| `design-md-chrome` | Browser-based design extraction |
| `open-design` | Open-source design tool |

### Website Generation
| Tool | Function | Priority |
|------|----------|----------|
| `Astro` | Static site generator (our primary) | ★★★ Primary |
| `ai-website-cloner-template` | Clone existing website designs | ★★★ Preferred cloner |
| `open-lovable` | AI-powered full site generation | ★★ Backup |
| `anime.js` | JavaScript animation library | ★★ Enhancement |

### Design Quality (Post-Generation)
| Tool | Function | Install |
|------|----------|---------|
| **Impeccable** ★★★ | UI polish `/polish`, audit `/audit`, distill `/distill` | `npx skills add pbakaus/impeccable` |
| **Better Icons** ★★★ | 200K+ icons, MCP server, 150+ collections | `npx better-icons setup` |
| **Taste Skill** ★★ | Premium aesthetic, anti-slop rules | `npx skills add Leonxlnx/taste-skill` |

### Deployment & Infrastructure
| Tool | Function |
|------|----------|
| Directus | Headless CMS (Docker) |
| Medusa | E-commerce platform (Docker) |
| Dokploy | Docker deployment platform |
| Baota Panel | Server management |
| Plausible/Tianji | Privacy-friendly analytics |

---

## Lessons Learned (2026-05-23, roamglade.com)

> Deployed a 38-page Scottish Highlands travel site to VPS with Baota panel. These are the pitfalls and fixes.

### Tech Stack: Astro 6 + Tailwind 4

| Issue | Cause | Fix |
|-------|-------|-----|
| **CSS not loading** | `global.css` not imported in `BaseLayout.astro` | Always add `import '../styles/global.css'` in the root layout frontmatter |
| **`@tailwindcss/vite` build error** | Astro 6 bundles Vite 8, but `@tailwindcss/vite@4.x` requires Vite 7 | Add `"overrides": { "vite": "^7" }` to `package.json` |
| **`@import url()` warning** | Google Fonts `@import` placed after `@theme` block (CSS spec violation) | Use `<link rel="preconnect">` + `<link href="...">` in `<head>` instead of CSS `@import` |
| **Tailwind 4 config** | No `tailwind.config.js` — configuration goes in CSS via `@theme {}` block | Define colors and fonts in `global.css`: `@theme { --color-primary: #xxx; --font-serif: '...'; }` |
| **Custom colors not working** | Tailwind 4 only auto-generates utilities for `--color-*` tokens | Name custom tokens `--color-{name}` (e.g. `--color-surface`, `--color-muted`) |
| **Astro 6 peer dependency conflicts** | `@astrojs/mdx@latest` requires astro@^6 but `@astrojs/tailwind@latest` requires astro@^5 | Check peer deps before install; use `@tailwindcss/vite` directly (not `@astrojs/tailwind`) |

### Images: China-hosted VPS

| Issue | Cause | Fix |
|-------|-------|-----|
| **All images 404/503** | `source.unsplash.com` blocked in China; `images.unsplash.com` CDN inaccessible from VPS | **Never** use external image CDNs for China-hosted sites. Download images locally, host on VPS |
| **Pexels scraping fails** | Pexels is JS-rendered; `requests.get()` returns empty HTML | Use Playwright browser to extract image URLs: `page.evaluate()` on search results |
| **Batch image download** | 36 pages × multiple images = 100+ unique images needed | Curate ~25 well-chosen images covering all categories; reuse across pages (hero-default for generic pages) |
| **Image URL mapping** | Unsplash search terms don't map 1:1 to filenames | Write a regex replacement script that maps search terms → local filenames; test with dry run first |

**Recommended image workflow for China VPS:**
1. `browser_run_code_unsafe` → visit 15-20 Pexels search URLs → `page.evaluate()` extract image URLs
2. Python script downloads images to `public/images/`
3. Replacement script swaps `source.unsplash.com/featured/...` → `/images/filename.jpg`
4. Upload images + rebuilt dist to VPS

### Deployment: Baota Panel + Existing Sites

| Issue | Cause | Fix |
|-------|-------|-----|
| **`nginx -t` fails on system nginx** | System `/etc/nginx/nginx.conf` doesn't exist; Baota uses `/www/server/nginx/` | Use `/www/server/nginx/sbin/nginx -t` and `/www/server/nginx/sbin/nginx -s reload` |
| **Site config not matching requests** | `server_name _` conflicts with default server block | Use `listen 80 default_server` OR a specific port (8080) to avoid conflicts |
| **`$uri` variables eaten by heredoc** | Bash `<< EOF` expands `$uri` as shell variable | Use Python SFTP `putfo(io.BytesIO(...))` instead of heredoc to write nginx config |
| **`systemctl restart nginx` dangerous** | Server has other live sites; restart = downtime | Always use **reload** (`-s reload`), never restart |
| **Python GBK encoding crash** | Windows console uses GBK; `✓` emoji causes `UnicodeEncodeError` | Add at top of every Python script: `sys.stdout.reconfigure(encoding='utf-8', errors='replace')` |
| **paramiko `exec_command` not waiting** | `exec_command` returns immediately; next command may run before mkdir completes | Call `.read()` on stdout to block until command finishes |

**Safe deployment sequence for Baota VPS:**
1. Build locally (`npm run build`)
2. Create tar in memory, upload via SFTP `putfo()`
3. Extract on server: `tar xzf site.tar.gz`
4. Write nginx config via SFTP (NOT heredoc)
5. `/www/server/nginx/sbin/nginx -t` → test
6. `/www/server/nginx/sbin/nginx -s reload` → apply
7. Verify: `curl -sI http://localhost:8080/en/`

### Domain & SSL

| Issue | Fix |
|-------|-----|
| Domain on Cloudflare, VPS in China | Use Cloudflare DNS proxy (orange cloud) + Flexible SSL mode — zero VPS cert config needed |
| Domain change requires many updates | Batch-replace: `astro.config.mjs` site URL, `seo.ts` SITE constant, all `.astro` files (brand name, email, footer) |
| Sitemap URLs wrong after domain change | `@astrojs/sitemap` auto-generates from `astro.config.mjs` → just rebuild |

### Content & Legal Compliance

- **No CMS needed for Phase 1** — 38 static pages work fine as `.astro` files; add Directus later when content volume justifies it
- **GDPR for EU visitors**: Privacy Policy + Cookie Policy + Terms & Conditions pages, plus a cookie consent banner with Accept/Reject
- **Cookie consent implementation**: Simple `localStorage`-based banner component, no external library needed
- **Anti-AI content writing**: Search 2-3 real articles per topic → synthesize → vary sentence length → add specific prices/times → use contractions → remove "Furthermore/In conclusion/whether you're..." patterns

### Claude Code Specific

- Use `mcp__playwright__browser_run_code_unsafe` for multi-page scraping (much faster than sequential `browser_navigate`)
- `WebSearch` + `WebFetch` for competitor research (Jina Reader `r.jina.ai` for content extraction)
- Skills to invoke: `frontend-design` for component design, `copywriting`/`copy-editing` for content quality
- Background bash tasks (`run_in_background`) for long-running npm install/build operations
- Pexels images extracted via browser; Unsplash API/source URLs unreliable in China

### Pexels Image Search Workflow (Detailed)

> For China-hosted VPS, external image CDNs don't work. You must download images locally and host them on the VPS.

**Step 1: Collect image URLs via browser automation**
```
Use mcp__playwright__browser_run_code_unsafe with a script that:
1. Loops through 15-20 search terms (e.g. "scottish highlands glencoe mountain")
2. Navigates to https://www.pexels.com/search/{term}/
3. page.evaluate() to extract <img srcset> from article elements
4. Returns the best quality URL (last item in srcset, w=1920)
5. Collects 2 images per search term = 30+ unique images
```
**Step 2: Download with Python**
- Map search terms → output filenames (e.g. "isle of skye quiraing" → `isle-of-skye-hero.jpg`)
- Use `requests.get()` with User-Agent header
- Save to `public/images/`

**Step 3: Replace URLs in .astro files**
- Write a regex script: `source.unsplash.com/featured/.../?{term}` → `/images/{mapped-name}.jpg`
- Process all `.astro` files recursively

**Step 4: Upload to VPS**
- SFTP `put()` each image to `/www/wwwroot/{site}/images/`
- Rebuild and upload dist

**Alternative image sources if Pexels is blocked:**
| Source | Status in China | Notes |
|--------|----------------|-------|
| Unsplash | ❌ 503 | `source.unsplash.com` blocked, `images.unsplash.com` also blocked |
| Pexels | ✅ (browser only) | JS-rendered, requires Playwright extraction |
| Pixabay | ❌ 403 | Blocked from both local and VPS |
| Picsum | ✅ | Random placeholder images, no topic matching |
| StockSnap.io | Try if Pexels fails | Not tested |

### Performance Optimization

| Issue | Fix | Impact |
|-------|-----|--------|
| **Images 700KB+ as JPEG** | Convert to WebP (Pillow: `img.save('name.webp', 'WEBP', quality=82)`) | 70-90% size reduction |
| **Hero image loads late** | Add `<link rel="preload" as="image" imagesrcset="..." imagesizes="100vw">` in `<head>` | LCP improvement |
| **Mobile loads 1920px images** | Generate 3 sizes (800w, 1200w, 1920w) + `<picture>` with `srcset` | Mobile bandwidth cut 85% |
| **All pages full-res images** | Create reusable `<Picture>` component that auto-generates WebP srcset from any `/images/x.jpg` path | Every page benefits |
| **JPEG + WebP both loading** | `<picture>` with `<source type="image/webp">` + JPEG `<img>` fallback — browser picks one | No double download |

**WebP conversion script pattern:**
```python
from PIL import Image
for jpg in glob('*.jpg'):
    img = Image.open(jpg)
    for w in [1920, 1200, 800]:
        h = int(img.size[1] * w / img.size[0])
        resized = img.resize((w, h), Image.LANCZOS)
        resized.save(f'webp/{name}-{w}w.webp', 'WEBP', quality=82)
```

### Cloudflare Optimization (for China-hosted VPS)

| Setting | Location | Value |
|---------|----------|-------|
| **SSL Mode** | SSL/TLS → Overview | **Flexible** (Cloudflare ↔ visitor HTTPS; Cloudflare ↔ origin HTTP port 80) |
| **Page Rule: Cache Images** | Rules → Page Rules | `roamglade.com/images/*` → Cache Everything, Edge TTL: 1 month |
| **Page Rule: Cache HTML** | Rules → Page Rules | `roamglade.com/*` → Cache Everything, Edge TTL: 2 hours |
| **Auto Minify** | Speed → Optimization | JS, CSS, HTML all ON (if not greyed out — Astro already minifies) |
| **Early Hints** | Speed → Optimization | ON (pre-pushes fonts + CSS links) |
| **Bot Fight Mode** | Security → Bots | **OFF** (can 403-block legitimate mobile users in China) |
| **Rocket Loader** | Speed → Optimization | **OFF** (breaks static Astro sites) |

**Cloudflare quirks:**
- After changing origin files, always **Purge Cache** (Caching → Configuration → Purge Everything)
- `cf-cache-status: DYNAMIC` on HTML is normal without Page Rules; images show `HIT` with proper rules
- Port 443 on origin: if using Flexible SSL, Cloudflare connects to origin port 80 — make sure your nginx listens on 80 for the domain
- When debugging: add `?nocache=1` to bypass Cloudflare cache

### SEO Submission Checklist

1. **Verify files accessible**: `robots.txt` (must reference correct sitemap URL), `sitemap-index.xml` (all 38 URLs use correct domain)
2. **Google Search Console**: Add property → HTML file verification (copy file to `public/`, rebuild, deploy) → Submit `sitemap-index.xml`
3. **Bing Webmaster Tools**: Import from Google Search Console (one click)
4. **Common gotcha**: `robots.txt` is a static file in `public/` — domain changes must be manual (not auto-generated like sitemap)

### Root Redirect (Astro i18n)

When using `prefixDefaultLocale: true`, the homepage is at `/en/` but `/` returns 404/403. Fix:
```astro
// src/pages/index.astro
---
return Astro.redirect('/en/', 301);
---
```

### Redirect Fix + Other Quick Fixes

| Problem | Solution |
|---------|----------|
| `/` returns 403 instead of redirecting | Create `src/pages/index.astro` with `Astro.redirect('/en/', 301)` |
| 403 on mobile but desktop works | Cloudflare Security → Bots → turn OFF Bot Fight Mode |
| Old domain in robots.txt after domain change | `public/robots.txt` is static — manually edit, rebuild, purge Cloudflare cache |
| Google verification via DNS fails | Use HTML file method: copy `.html` to `public/`, rebuild, deploy, click Verify |

### Docker Services on VPS (Directus CMS + Umami Analytics)

**Pre-check before installing Docker:**
- VPS needs ≥ 2GB RAM, ≥ 10GB free disk
- Ubuntu 24.04: `apt-get install -y docker.io docker-compose-v2` (not `get.docker.com` script — may have dpkg conflicts)
- If dpkg errors on unrelated packages (nginx, python3-certbot), ignore — Docker still installs

**docker-compose.yml — three services:**
```yaml
services:
  directus:     # CMS on port 8055, SQLite
  umami:        # Analytics on port 3000 (map to host 8000)
  db:           # PostgreSQL for Umami
```

**Persistent volumes (critical for China VPS):**
| Service | Host Path | Container Path | Data |
|---------|-----------|----------------|------|
| Directus | `./directus-data` | `/directus/database` | SQLite DB |
| Directus | `./directus-uploads` | `/directus/uploads` | Uploaded files |
| Umami DB | `./umami-db` | `/var/lib/postgresql/data` | Analytics data |

**Umami port gotcha:** Container listens on 3000, map to host 8000 (`"8000:3000"` NOT `"8000:8000"`).

**Directus PUBLIC_URL gotcha:** Changes after first boot are stored in DB, not re-read from env. If changing from `/cms` path to subdomain, delete `directus-data/` and re-initialize.

**Subdomain setup for both services:**
1. Cloudflare DNS → CNAME `cms` → `roamglade.com` (proxy ON)
2. Cloudflare DNS → CNAME `umami` → `roamglade.com` (proxy ON)
3. Baota Nginx: create separate `.conf` for each subdomain, proxy to `127.0.0.1:8055` and `127.0.0.1:8000`
4. `PUBLIC_URL: https://cms.roamglade.com` (no trailing path — subdomain handles routing)

**Umami tracking code pattern:**
```html
<script defer src="https://roamglade.com/umami/script.js" data-website-id="UUID"></script>
```
- Get website ID from Umami admin → Settings → Websites → Edit → Tracking code
- Or create via API: `POST /api/websites` with name + domain, get ID from response
- Default login: admin / umami (change immediately)

**Directus access details:**
- URL: `https://cms.roamglade.com` → auto-redirects to `/admin/login`
- First-run credentials set via env: `ADMIN_EMAIL` + `ADMIN_PASSWORD`
- Baota Docker manager can control all three containers

### Full Deployment Checklist (China VPS + Cloudflare)

1. ✅ Astro site built and uploaded to `/www/wwwroot/{site}/`
2. ✅ Baota nginx config created in `/www/server/panel/vhost/nginx/`
3. ✅ Cloudflare DNS A records (root + www) with proxy ON
4. ✅ Cloudflare SSL set to Flexible
5. ✅ Cloudflare Page Rules for caching
6. ✅ `robots.txt` + `sitemap-index.xml` verified
7. ✅ Google Search Console submitted
8. ✅ Docker services running (Directus + Umami) with persistent volumes
9. ✅ Subdomain CNAME records + nginx proxy for cms + umami
10. ✅ Umami tracking script in BaseLayout
11. ✅ Root redirect (`/` → `/en/` 301)
12. ✅ Cookie consent + GDPR pages
13. ✅ WebP images + responsive srcset + hero preload

### SEO Content Writing (Anti-Duplicate Methodology)

> Google doesn't penalize "duplicate content" — it filters pages that are substantially identical. Synthesis-based writing is safe if every article has unique structure, data, and voice.

**Research → Synthesize → Personalize pipeline:**
1. `WebSearch` 2-3 articles on the same topic
2. Extract facts, data, and unique angles from each
3. Rewrite in your own sentence structure (never copy-paste)
4. Add original data: price tables, comparison charts, cost breakdowns
5. Include specific numbers (prices, distances, times) not found in any single source
6. Internal links to your own pillar pages (creates unique context)

**The Editor's Note — 100% duplicate-proof:**
Every article must end with a personal Editor's Note block:
```html
<div class="border-l-4 border-primary pl-5 mt-10">
  <p class="text-xs text-accent uppercase">Editor's Note</p>
  <p class="text-sm">[First-person anecdote with specific details — date, location, weather, what happened]</p>
</div>
```
This is impossible to duplicate because it's personal experience. Key elements:
- Specific date or season ("October 2024", "my third trip")
- Named location ("the Clachaig Inn", "Robertson's Farm at 8am")
- Personal mistake or observation
- Self-deprecating tone preferred

**Anti-AI markers to remove:**
- "whether you're a seasoned traveler or..." (delete entirely)
- "nestled in the heart of..." → describe what it actually looks like
- "breathtaking" / "iconic" / "must-see" — use once per article max
- Perfect paragraph symmetry (vary length: one sentence. Then three. Then two.)
- "In conclusion" / "Furthermore" / "Moreover" — never use

### Site Structure: Journal vs Blog

For travel/content sites:
- **Don't call it "Blog"** — use **"Journal"** or **"Stories"**
- Articles live under `/en/journal/` with their own subdirectories
- Each article is just `index.astro` in its own folder → clean URL: `/en/journal/nc500-in-winter/`
- Journal index lists all articles as cards with tags
- Navigation label changes: `Blog` → `Journal` in `navigation.ts`, update `sitemap.astro`, footer references

**Article tags for the journal index:**
Group articles by tag for the card display: `Road Trips`, `Isle of Skye`, `Accommodation`, `Planning`, `Personal Stories`, `Food & Drink`, `Photography`, `Wildlife`

### Content Publishing Strategy

**Recommended cadence for new sites:**
| Phase | Frequency | Purpose |
|-------|-----------|---------|
| Launch | 10-20 articles at once | Populate site with enough content for crawlers |
| Months 1-3 | 2 articles/week (Tue + Thu, fixed UTC times) | Google learns your publishing rhythm |
| Months 4+ | 1-2 articles/week, based on analytics | Double down on what's getting traffic |

**Critical:** Fixed publishing times matter more than you'd think. Googlebot learns patterns. Consistently publishing Tuesday 09:00 UTC trains the crawler to check your site at that time.

### Destination Expansion Strategy

**Start with 5-7 core destinations → expand to 15-20 for broader keyword coverage:**
- Phase 1 (core): Skye, Glencoe, Loch Ness, Cairngorms, Fort William
- Phase 2 (expand): Torridon, Assynt, Oban, Loch Lomond, Perthshire, Moray Coast, Ullapool, Applecross, Isle of Mull, Ardnamurchan

**Each destination follows the same template:**
1. Hero with tagline + image
2. Quick-info box (4 stats: best time, stay duration, getting there, known for)
3. Why-visit section (2-3 paragraphs)
4. Main attractions (3-5 items, each with 💡 tip)
5. Newsletter CTA

Template is compact enough to batch-create 4-5 destinations per session. Each page targets its own long-tail keyword cluster without competing with other pages.

### Article Topic Selection by Keyword Intent

| Intent | Article Type | Example |
|--------|-------------|---------|
| Informational | "Best X in the Highlands" | Best Beaches, Best Bothies |
| Practical | "How to X" | How to Read Weather, Packing Mistakes |
| Comparative | "X vs Y" | Glencoe vs Skye |
| Seasonal | "X in [Season]" | Autumn Photography, Spring Wildflowers |
| Cultural | "X Explained" | Gaelic Phrases |
| Personal narrative | "What I Learned" | 10 Days of Scottish Rain |

---

## Notes for Agent Compatibility

### Hermes
- This skill is fully Hermes-compatible (SKILL.md + YAML frontmatter)
- Hermes can use Tavily for search, Jina Reader for crawling
- MCP tools (mcp-chrome, etc.) can be connected for browser automation
- Memory system: use Hermes built-in memory for cross-session state

### Claude Code
- Drop this folder into `.claude/skills/` or reference with `--skill`
- Claude Code can execute scripts directly
- Browser automation via Stagehand integration

### Codex / Antigravity
- Compatible as a standard skill folder
- Scripts can be invoked via `run_command`
- Web search and URL fetching available natively

### Cross-Platform State
- All intermediate artifacts are saved as files (`.md`, `.yaml`, `.json`)
- Any agent can resume from any phase by reading existing artifacts
- No proprietary state format — everything is human-readable

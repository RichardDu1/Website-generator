# SEOplant — Tool Registry

This document maps every tool in your GitHub Stars to its role in the SEOplant pipeline.

## Quick Lookup

| Phase | Primary Tools | Fallback Tools |
|-------|-------------|----------------|
| **1. Competitor Intel** | Jina Reader/Search, Scrapling | spider-rs, ScrapeGraphAI, CamoFox |
| **2. SEO/GEO** | geo-seo-claude, claude-seo, seomachine | toprank, marketingskills |
| **3. Design** | dembrandt, superdesign, awesome-design-md | ui-ux-pro-max-skill, design-md-chrome |
| **4. Site Gen** | Astro, ai-website-cloner-template | open-lovable, anime.js |
| **4b. Icons** | **Better Icons MCP** | — |
| **4c. Polish** | **Impeccable** (/polish /audit /distill) | **Taste Skill** (optional premium) |
| **5. Deploy** | Baota + Nginx, Dokploy | Docker Compose manual |

---

## Tool Details

### Crawling Stack (Escalation Ladder)

```
Level 1: Jina Reader
  URL: https://r.jina.ai/{url}
  Cost: Free
  Limits: Rate limited
  Best for: Simple page content extraction

Level 2: spider-rs
  Repo: https://github.com/spider-rs/spider
  Lang: Rust (binary)
  Best for: High-performance batch crawling

Level 3: Scrapling
  Repo: https://github.com/D4Vinci/Scrapling
  Lang: Python
  Best for: Anti-bot bypass, intelligent extraction

Level 4: ScrapeGraphAI
  Repo: https://github.com/ScrapeGraphAI/Scrapegraph-ai
  Lang: Python
  Best for: AI-driven structured data extraction

Level 5: CamoFox
  Repo: https://github.com/jo-inc/camofox-browser
  Best for: Stealth browsing when all else fails

Level 6: Stagehand
  Repo: https://github.com/browserbase/stagehand
  Best for: Full browser automation orchestration
```

### SEO Skills

```
geo-seo-claude
  Repo: https://github.com/zubair-trabzada/geo-seo-claude
  Function: GEO + traditional SEO combined strategy
  Key: AI search engine citation optimization

claude-seo
  Repo: https://github.com/AgriciDaniel/claude-seo
  Function: E-E-A-T content quality + Schema markup
  Key: Semantic topic clustering, structured data

seomachine
  Repo: https://github.com/TheCraigHewitt/seomachine
  Function: Full pipeline: research → content → optimize → publish
  Key: End-to-end content SEO automation

toprank
  Repo: https://github.com/nowork-studio/toprank
  Function: SEO rank tracking
  Key: Post-launch monitoring

marketingskills
  Repo: https://github.com/coreyhaines31/marketingskills
  Function: Comprehensive marketing strategy
  Key: Social media + content marketing plans
```

### Design Tools

```
dembrandt (npm)
  Install: npx dembrandt {url}
  Function: Extract DESIGN.md from any website URL
  Output: Structured design tokens (colors, fonts, spacing)

superdesign
  Repo: https://github.com/superdesigndev/superdesign
  Function: AI Product Design Agent
  Key: Automated design generation and iteration

awesome-design-md
  Repo: https://github.com/VoltAgent/awesome-design-md
  Function: Collection of DESIGN.md files for major brands
  Key: Design reference library (Stripe, Linear, Vercel, etc.)

ui-ux-pro-max-skill
  Repo: https://github.com/nextlevelbuilder/ui-ux-pro-max-skill
  Function: AI-powered UI/UX design skill for Claude Code
  Key: Design intelligence and component generation

design-md-chrome
  Repo: https://github.com/bergside/design-md-chrome
  Function: Chrome extension to extract DESIGN.md
  Key: Manual extraction fallback (requires browser)

open-design
  Repo: https://github.com/nexu-io/open-design
  Function: Open-source design tool
  Key: Programmatic design manipulation
```

### Website Generation

```
Astro (primary)
  URL: https://astro.build
  Function: Static site generator
  Key: Our primary build tool, i18n support, MDX

open-lovable
  Repo: https://github.com/firecrawl/open-lovable
  Function: AI-powered full website generation
  Key: Can generate entire sites from description

ai-website-cloner-template
  Repo: https://github.com/JCodesMore/ai-website-cloner-template
  Function: Clone existing website designs with AI
  Key: "See a site you like → clone its design"

anime.js
  Repo: https://github.com/juliangarnier/anime
  Function: JavaScript animation library
  Key: Micro-animations, scroll effects, page transitions
```

### Design Quality (Post-Generation)

```
Impeccable ★★★ REQUIRED
  Repo: https://github.com/pbakaus/impeccable
  Install: npx skills add pbakaus/impeccable
  Function: 20+ design commands for UI polish, audit, distill
  Key commands:
    /polish  — spacing, alignment, typography micro-details
    /audit   — design inconsistency detection
    /distill — remove visual noise, simplify
    /typeset — typography optimization
    /arrange — layout composition
  When: Run after all pages are generated (Phase 4.8)

Better Icons ★★★ REQUIRED
  Repo: https://github.com/better-auth/better-icons
  Install: npx better-icons setup
  Function: MCP server for 200K+ icons from 150+ collections
  Collections: Lucide, Heroicons, Phosphor, Tabler, Material, Simple Icons
  Usage: npx better-icons search {query} / npx better-icons get {collection}:{name}
  When: During site structure generation (Phase 4.2b)

Taste Skill ★★ RECOMMENDED
  Repo: https://github.com/Leonxlnx/taste-skill
  Install: npx skills add Leonxlnx/taste-skill
  Function: Premium aesthetic enforcement + anti-slop rules
  Variants:
    soft-skill       — luxury, "looks expensive" (travel/brand sites)
    minimalist-skill — clean, minimal (tech/tool sites)
    brutalist-skill  — bold, raw (creative/art sites)
    redesign-skill   — audit & improve existing code
  Tunable params: DESIGN_VARIANCE, MOTION_INTENSITY, VISUAL_DENSITY
  When: After Impeccable polish, for premium feel (Phase 4.8 optional)
```

### Infrastructure

```
Directus
  URL: https://directus.io
  Function: Headless CMS (self-hosted, Docker)
  Key: Content management backend

Medusa
  Repo: https://github.com/medusajs/medusa
  Function: Open-source e-commerce platform
  Key: Product management for e-commerce sites

Dokploy
  Function: Docker deployment platform
  Key: Simplified Docker management on VPS

Baota Panel
  Function: Server management panel
  Key: Nginx config, SSL, monitoring

Plausible / Tianji
  Function: Privacy-friendly analytics
  Key: Self-hosted, no cookie consent needed
```

### Agent Enhancement

```
superpowers — Claude Code enhanced capabilities
obra/superpowers

one-api — Unified API gateway for multiple LLMs
songquanpeng/one-api

rtk — Token compression for cost optimization
rtk-ai/rtk

agentmemory — Persistent agent memory
rohitg00/agentmemory

hermeshub — Hermes skill distribution
amanning3390/hermeshub
```

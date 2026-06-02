# website-generator

End-to-end website generation skill for Claude Code, Hermes, and Codex. Takes a keyword/description → produces a production-ready static website with SEO/GEO optimization, competitive design, and deployment configuration.

## Quick Start

```
Help me generate a [travel|ecommerce|saas|portfolio|docs] website about [keyword]
```

The skill orchestrates 5 phases:
1. **Competitor Intelligence** — analyze top 5 competitors
2. **SEO/GEO Planning** — keyword research, content calendar, llms.txt
3. **Design Intelligence** — extract design tokens from real websites, fuse into original DESIGN.md
4. **Site Generation** — Astro + Tailwind, all components, content pages, SEO injection
5. **Deployment** — Nginx config, Docker Compose, rsync deployment

## Supported Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| **Claude Code** | Full support | Native WebSearch, Playwright MCP, file ops |
| **Hermes** | Full support | Tavily search, Jina Reader, skills system |
| **Codex / Antigravity** | Compatible | Standard skill folder format |

## Dependencies

### Required (all platforms)
- **Node.js** ≥ 18 (`npm`, `npx`)
- **Python** ≥ 3.10 (`paramiko` for SSH, `Pillow` for images, `requests`)

### Platform-specific
| Dependency | Claude Code | Hermes | Codex |
|------------|:-----------:|:------:|:-----:|
| Playwright MCP | ✅ Built-in | ⚠️ mcp-chrome | ❌ skip |
| Web Search | ✅ WebSearch | ✅ Tavily/DuckDuckGo | ✅ native |
| Jina Reader (`r.jina.ai`) | ✅ WebFetch | ✅ native | ✅ native |
| `paramiko` (Python) | ✅ pip install | ✅ pip install | ✅ pip install |
| `Pillow` (Python) | ✅ pip install | ✅ pip install | ✅ pip install |

### Optional (server-side)
- **Docker** + **docker-compose** — Directus CMS, Umami analytics
- **Cloudflare** account — DNS, SSL, CDN caching

## Installation

### Claude Code
```bash
cp -r website-generator ~/.claude/skills/website-generator
```

### Hermes (server)
```bash
scp -r website-generator user@vps:~/.hermes/skills/website-generator
# Or via Baota panel: upload to /root/.hermes/skills/
```

### Codex / Antigravity
Drop the `website-generator/` folder into the skills directory.

## File Structure

```
website-generator/
├── SKILL.md                    # Main skill file (all 5 phases)
├── README.md                   # This file
├── LICENSE                     # MIT
├── scripts/
│   ├── competitor_intel.py     # Phase 1: competitor research
│   ├── seo_engine.py           # Phase 2: SEO keyword planning
│   ├── design_intel.py         # Phase 3: design extraction
│   ├── site_builder.py         # Phase 4: Astro scaffolding
│   └── deployer.py             # Phase 5: deployment configs
├── references/
│   └── tool-registry.md        # Complete tool capability registry
├── design-library/
│   └── index.json              # Local design reference index
└── SEO-CONTENT-PLAN.md         # Sample: Scottish Highlands SEO plan
```

## License

MIT — see [LICENSE](LICENSE) for details.

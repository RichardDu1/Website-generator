"""
SEOplant Site Builder — Phase 4 of the pipeline.

Scaffolds an Astro-based static website project, applies design tokens
from DESIGN.md, injects SEO metadata, and configures optional modules.

Dependencies: none (generates files, doesn't install packages itself)
"""

import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path
from textwrap import dedent
from typing import Optional


# ---------------------------------------------------------------------------
# 1. Initialize Project Structure
# ---------------------------------------------------------------------------

def generate_project_structure(
    project_dir: str,
    site_name: str = "My Website",
    languages: list[str] = None,
    modules: dict = None,
) -> dict:
    """Generate the complete Astro project directory structure.

    Creates all directories and placeholder files.

    Returns:
        Dict with created_dirs and created_files lists
    """
    languages = languages or ["en"]
    modules = modules or {}
    base = Path(project_dir)
    created = {"dirs": [], "files": []}

    # Directory structure
    dirs = [
        "src/layouts",
        "src/components",
        "src/pages/blog",
        "src/content/blog",
        "src/styles",
        "src/scripts",
        "src/data",
        "src/i18n",
        "public/images",
        "deploy",
    ]

    # Add language directories
    for lang in languages[1:]:  # Skip primary language (root)
        dirs.append(f"src/pages/{lang}")
        dirs.append(f"src/pages/{lang}/blog")

    # Module-specific directories
    if modules.get("cms"):
        dirs.append("src/lib")
    if modules.get("map"):
        dirs.append("src/components/map")

    for d in dirs:
        path = base / d
        path.mkdir(parents=True, exist_ok=True)
        created["dirs"].append(str(path))

    return created


# ---------------------------------------------------------------------------
# 2. Generate Astro Config
# ---------------------------------------------------------------------------

def generate_astro_config(
    site_url: str = "https://example.com",
    languages: list[str] = None,
    modules: dict = None,
) -> str:
    """Generate astro.config.mjs content."""
    languages = languages or ["en"]
    modules = modules or {}

    integrations = ["sitemap()"]
    imports = ["import { defineConfig } from 'astro/config';",
               "import sitemap from '@astrojs/sitemap';"]

    if modules.get("mdx"):
        imports.append("import mdx from '@astrojs/mdx';")
        integrations.append("mdx()")

    return f"""{chr(10).join(imports)}

export default defineConfig({{
  site: '{site_url}',
  integrations: [{', '.join(integrations)}],
  i18n: {{
    defaultLocale: '{languages[0]}',
    locales: {json.dumps(languages)},
    routing: {{
      prefixDefaultLocale: false,
    }},
  }},
  markdown: {{
    shikiConfig: {{
      theme: 'github-dark',
    }},
  }},
}});
"""


# ---------------------------------------------------------------------------
# 3. Generate CSS from DESIGN.md
# ---------------------------------------------------------------------------

def generate_global_css(design_tokens: dict = None) -> str:
    """Convert DESIGN.md tokens into CSS custom properties."""
    tokens = design_tokens or {}
    colors = tokens.get("colors", {})
    typo = tokens.get("typography", {})
    spacing = tokens.get("spacing", {})

    return f"""/* ==========================================================================
   Global Styles — Generated from DESIGN.md
   ========================================================================== */

/* Design Tokens */
:root {{
  /* Colors */
  --color-primary: {colors.get('primary', '#3B82F6')};
  --color-secondary: {colors.get('secondary', '#10B981')};
  --color-accent: {colors.get('accent', '#F59E0B')};
  --color-background: {colors.get('background', '#ffffff')};
  --color-surface: {colors.get('surface', '#f8fafc')};
  --color-text: {colors.get('text', '#1e293b')};
  --color-text-muted: {colors.get('text-muted', '#64748b')};
  --color-border: #e2e8f0;

  /* Typography */
  --font-heading: '{typo.get('heading', 'Inter')}', system-ui, -apple-system, sans-serif;
  --font-body: '{typo.get('body', 'Inter')}', system-ui, -apple-system, sans-serif;
  --font-mono: '{typo.get('mono', 'JetBrains Mono')}', ui-monospace, monospace;

  /* Spacing */
  --space-xs: 4px;
  --space-sm: 8px;
  --space-md: 16px;
  --space-lg: 24px;
  --space-xl: 32px;
  --space-2xl: 48px;
  --space-3xl: 64px;
  --space-4xl: 96px;

  /* Layout */
  --max-width: 1280px;
  --border-radius: 8px;
  --border-radius-lg: 16px;

  /* Transitions */
  --transition-fast: 150ms ease;
  --transition-base: 250ms ease;
  --transition-slow: 400ms ease;
}}

/* Dark mode */
@media (prefers-color-scheme: dark) {{
  :root {{
    --color-background: #0f172a;
    --color-surface: #1e293b;
    --color-text: #f1f5f9;
    --color-text-muted: #94a3b8;
    --color-border: #334155;
  }}
}}

/* Base Reset */
*, *::before, *::after {{
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}}

html {{
  font-size: 16px;
  scroll-behavior: smooth;
  -webkit-text-size-adjust: 100%;
}}

body {{
  font-family: var(--font-body);
  color: var(--color-text);
  background-color: var(--color-background);
  line-height: 1.7;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}}

/* Typography */
h1, h2, h3, h4, h5, h6 {{
  font-family: var(--font-heading);
  font-weight: 700;
  line-height: 1.3;
  color: var(--color-text);
}}

h1 {{ font-size: 2.5rem; margin-bottom: var(--space-lg); }}
h2 {{ font-size: 2rem; margin-bottom: var(--space-md); }}
h3 {{ font-size: 1.5rem; margin-bottom: var(--space-md); }}
h4 {{ font-size: 1.25rem; margin-bottom: var(--space-sm); }}

p {{ margin-bottom: var(--space-md); }}

a {{
  color: var(--color-primary);
  text-decoration: none;
  transition: color var(--transition-fast);
}}
a:hover {{ color: var(--color-secondary); }}

img {{
  max-width: 100%;
  height: auto;
  display: block;
}}

code {{
  font-family: var(--font-mono);
  background: var(--color-surface);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.875em;
}}

/* Layout Utilities */
.container {{
  max-width: var(--max-width);
  margin-inline: auto;
  padding-inline: var(--space-lg);
}}

.section {{
  padding-block: var(--space-3xl);
}}

.grid {{
  display: grid;
  gap: var(--space-lg);
}}

.flex {{
  display: flex;
  gap: var(--space-md);
}}

/* Components */
.card {{
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--border-radius);
  padding: var(--space-lg);
  transition: transform var(--transition-base), box-shadow var(--transition-base);
}}
.card:hover {{
  transform: translateY(-2px);
  box-shadow: 0 8px 25px rgba(0,0,0,0.1);
}}

.btn {{
  display: inline-flex;
  align-items: center;
  gap: var(--space-sm);
  padding: var(--space-sm) var(--space-lg);
  border: none;
  border-radius: var(--border-radius);
  font-family: var(--font-body);
  font-weight: 600;
  font-size: 1rem;
  cursor: pointer;
  transition: all var(--transition-fast);
  text-decoration: none;
}}

.btn-primary {{
  background: var(--color-primary);
  color: white;
}}
.btn-primary:hover {{
  opacity: 0.9;
  transform: translateY(-1px);
  color: white;
}}

.btn-secondary {{
  background: transparent;
  color: var(--color-primary);
  border: 2px solid var(--color-primary);
}}
.btn-secondary:hover {{
  background: var(--color-primary);
  color: white;
}}

/* Responsive */
@media (max-width: 768px) {{
  h1 {{ font-size: 2rem; }}
  h2 {{ font-size: 1.5rem; }}
  .section {{ padding-block: var(--space-2xl); }}
}}
"""


# ---------------------------------------------------------------------------
# 4. Generate Base Layout
# ---------------------------------------------------------------------------

def generate_base_layout(
    site_name: str = "My Website",
    google_fonts: list[str] = None,
) -> str:
    """Generate src/layouts/BaseLayout.astro."""
    google_fonts = google_fonts or ["Inter"]
    font_params = "&".join([f"family={f.replace(' ', '+')}" for f in google_fonts])

    return f"""---
import '../styles/global.css';
import SEOHead from '../components/SEOHead.astro';
import Header from '../components/Header.astro';
import Footer from '../components/Footer.astro';

interface Props {{
  title: string;
  description?: string;
  image?: string;
  canonicalUrl?: string;
  type?: string;
}}

const {{ title, description = '', image = '', canonicalUrl = '', type = 'website' }} = Astro.props;
---

<!DOCTYPE html>
<html lang={{Astro.currentLocale || 'en'}}>
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?{font_params}&display=swap" rel="stylesheet" />
  <SEOHead
    title={{title}}
    description={{description}}
    image={{image}}
    canonicalUrl={{canonicalUrl}}
    type={{type}}
  />
</head>
<body>
  <Header siteName="{site_name}" />
  <main>
    <slot />
  </main>
  <Footer siteName="{site_name}" />
</body>
</html>
"""


# ---------------------------------------------------------------------------
# 5. Generate SEO Head Component
# ---------------------------------------------------------------------------

def generate_seo_head(site_name: str = "My Website", site_url: str = "https://example.com") -> str:
    """Generate src/components/SEOHead.astro."""
    return f"""---
interface Props {{
  title: string;
  description?: string;
  image?: string;
  canonicalUrl?: string;
  type?: string;
}}

const {{ title, description = '', image = '', canonicalUrl = '', type = 'website' }} = Astro.props;
const fullTitle = title ? `${{title}} | {site_name}` : '{site_name}';
const url = canonicalUrl || Astro.url.href;
const ogImage = image || '{site_url}/og-default.png';
---

<title>{{fullTitle}}</title>
<meta name="description" content={{description}} />
<link rel="canonical" href={{url}} />

<!-- Open Graph -->
<meta property="og:type" content={{type}} />
<meta property="og:title" content={{fullTitle}} />
<meta property="og:description" content={{description}} />
<meta property="og:url" content={{url}} />
<meta property="og:image" content={{ogImage}} />
<meta property="og:site_name" content="{site_name}" />

<!-- Twitter -->
<meta name="twitter:card" content="summary_large_image" />
<meta name="twitter:title" content={{fullTitle}} />
<meta name="twitter:description" content={{description}} />
<meta name="twitter:image" content={{ogImage}} />

<!-- Additional SEO -->
<meta name="robots" content="index, follow" />
<meta name="generator" content={{Astro.generator}} />
"""


# ---------------------------------------------------------------------------
# 6. Generate Header/Footer Components
# ---------------------------------------------------------------------------

def generate_header(site_name: str = "My Website", nav_items: list[dict] = None) -> str:
    """Generate src/components/Header.astro."""
    nav_items = nav_items or [
        {"label": "Home", "href": "/"},
        {"label": "Blog", "href": "/blog"},
        {"label": "About", "href": "/about"},
        {"label": "Contact", "href": "/contact"},
    ]

    nav_html = "\n".join([
        f'      <a href="{item["href"]}" class="nav-link">{item["label"]}</a>'
        for item in nav_items
    ])

    return f"""---
interface Props {{
  siteName?: string;
}}
const {{ siteName = '{site_name}' }} = Astro.props;
---

<header class="site-header">
  <nav class="container flex" style="justify-content: space-between; align-items: center; padding-block: var(--space-md);">
    <a href="/" class="site-logo" style="font-family: var(--font-heading); font-size: 1.5rem; font-weight: 700; color: var(--color-text); text-decoration: none;">
      {{siteName}}
    </a>
    <div class="nav-links flex" style="gap: var(--space-lg);">
{nav_html}
    </div>
  </nav>
</header>

<style>
  .site-header {{
    border-bottom: 1px solid var(--color-border);
    position: sticky;
    top: 0;
    background: var(--color-background);
    z-index: 100;
    backdrop-filter: blur(8px);
  }}
  .nav-link {{
    color: var(--color-text-muted);
    font-weight: 500;
    transition: color var(--transition-fast);
  }}
  .nav-link:hover {{
    color: var(--color-primary);
  }}
</style>
"""


def generate_footer(site_name: str = "My Website") -> str:
    """Generate src/components/Footer.astro."""
    return f"""---
interface Props {{
  siteName?: string;
}}
const {{ siteName = '{site_name}' }} = Astro.props;
---

<footer class="site-footer section">
  <div class="container" style="text-align: center;">
    <p style="color: var(--color-text-muted);">
      &copy; {{new Date().getFullYear()}} {{siteName}}. All rights reserved.
    </p>
  </div>
</footer>

<style>
  .site-footer {{
    border-top: 1px solid var(--color-border);
    padding-block: var(--space-xl);
  }}
</style>
"""


# ---------------------------------------------------------------------------
# 7. Generate Index Page
# ---------------------------------------------------------------------------

def generate_index_page(
    site_name: str = "My Website",
    keyword: str = "website",
    description: str = "",
) -> str:
    """Generate src/pages/index.astro."""
    return f"""---
import BaseLayout from '../layouts/BaseLayout.astro';
---

<BaseLayout
  title="Home"
  description="{description or f'Welcome to {site_name} — your guide to {keyword}'}"
>
  <!-- Hero Section -->
  <section class="section" style="text-align: center; padding-block: var(--space-4xl);">
    <div class="container">
      <h1>Welcome to {site_name}</h1>
      <p style="font-size: 1.25rem; color: var(--color-text-muted); max-width: 600px; margin-inline: auto;">
        Your comprehensive guide to {keyword}. Discover destinations, tips, and everything you need to know.
      </p>
      <div style="margin-top: var(--space-xl); display: flex; gap: var(--space-md); justify-content: center;">
        <a href="/blog" class="btn btn-primary">Explore Guides</a>
        <a href="/about" class="btn btn-secondary">Learn More</a>
      </div>
    </div>
  </section>

  <!-- Features Section -->
  <section class="section" style="background: var(--color-surface);">
    <div class="container">
      <h2 style="text-align: center;">What We Offer</h2>
      <div class="grid" style="grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); margin-top: var(--space-xl);">
        <div class="card">
          <h3>📍 Destinations</h3>
          <p>Detailed guides to the most amazing places to visit.</p>
        </div>
        <div class="card">
          <h3>🗺️ Itineraries</h3>
          <p>Carefully planned routes for every type of traveler.</p>
        </div>
        <div class="card">
          <h3>💡 Tips & Advice</h3>
          <p>Insider knowledge to make your journey unforgettable.</p>
        </div>
      </div>
    </div>
  </section>
</BaseLayout>
"""


# ---------------------------------------------------------------------------
# 8. Parse DESIGN.md tokens
# ---------------------------------------------------------------------------

def parse_design_md(design_md_path: str) -> dict:
    """Parse a DESIGN.md file and extract tokens from YAML frontmatter.

    Returns:
        Dict of design tokens
    """
    try:
        content = Path(design_md_path).read_text(encoding="utf-8")
    except FileNotFoundError:
        return {}

    # Extract YAML frontmatter
    frontmatter_match = re.search(r'^---\s*\n(.+?)\n---', content, re.S)
    if not frontmatter_match:
        return {}

    yaml_str = frontmatter_match.group(1)
    tokens = {}

    # Simple YAML parsing (avoid external dependency)
    # Extract colors
    colors = {}
    for match in re.finditer(r'(\w[\w-]*):\s*"(#[0-9a-fA-F]{3,8})"', yaml_str):
        colors[match.group(1)] = match.group(2)

    # Extract typography
    typo = {}
    for match in re.finditer(r'(heading|body|mono):\s*"([^"]+)"', yaml_str):
        typo[match.group(1)] = match.group(2)

    tokens["colors"] = colors
    tokens["typography"] = typo

    # Extract name
    name_match = re.search(r'name:\s*"([^"]+)"', yaml_str)
    if name_match:
        tokens["name"] = name_match.group(1)

    return tokens


# ---------------------------------------------------------------------------
# 9. Scaffold Full Project
# ---------------------------------------------------------------------------

def scaffold_project(
    project_dir: str,
    site_name: str = "My Website",
    keyword: str = "website",
    site_url: str = "https://example.com",
    languages: list[str] = None,
    modules: dict = None,
    design_md_path: str = None,
) -> dict:
    """Scaffold a complete Astro project with all generated files.

    This is the main orchestration function for Phase 4.

    Returns:
        Dict with list of created files
    """
    languages = languages or ["en"]
    modules = modules or {}
    base = Path(project_dir)
    created_files = []

    # Parse DESIGN.md if available
    design_tokens = {}
    if design_md_path:
        design_tokens = parse_design_md(design_md_path)

    # Create directory structure
    generate_project_structure(project_dir, site_name, languages, modules)

    # Generate and write files
    files = {
        "astro.config.mjs": generate_astro_config(site_url, languages, modules),
        "src/styles/global.css": generate_global_css(design_tokens),
        "src/layouts/BaseLayout.astro": generate_base_layout(
            site_name,
            design_tokens.get("typography", {}).get("fonts", ["Inter"]),
        ),
        "src/components/SEOHead.astro": generate_seo_head(site_name, site_url),
        "src/components/Header.astro": generate_header(site_name),
        "src/components/Footer.astro": generate_footer(site_name),
        "src/pages/index.astro": generate_index_page(site_name, keyword),
    }

    # i18n files
    for lang in languages:
        i18n_content = json.dumps({
            "nav.home": "Home",
            "nav.blog": "Blog",
            "nav.about": "About",
            "nav.contact": "Contact",
            "footer.copyright": f"© {datetime.now().year} {site_name}",
        }, indent=2, ensure_ascii=False)
        files[f"src/i18n/{lang}.json"] = i18n_content

    # Package.json
    deps = {
        "astro": "^5.0.0",
        "@astrojs/sitemap": "^3.0.0",
    }
    if modules.get("mdx"):
        deps["@astrojs/mdx"] = "^3.0.0"

    files["package.json"] = json.dumps({
        "name": site_name.lower().replace(" ", "-"),
        "type": "module",
        "version": "1.0.0",
        "scripts": {
            "dev": "astro dev",
            "build": "astro build",
            "preview": "astro preview",
        },
        "dependencies": deps,
    }, indent=2)

    # robots.txt
    files["public/robots.txt"] = f"""User-agent: *
Allow: /
Sitemap: {site_url}/sitemap-index.xml
"""

    # Write all files
    for rel_path, content in files.items():
        file_path = base / rel_path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content, encoding="utf-8")
        created_files.append(str(rel_path))

    return {"created_files": created_files, "project_dir": str(base)}


# ---------------------------------------------------------------------------
# CLI Interface
# ---------------------------------------------------------------------------

def main():
    # Fix Windows GBK encoding crash when printing emoji / Unicode
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')

    if len(sys.argv) < 2:
        print("""
Usage:
  python site_builder.py scaffold ./my-site --name "My Site" --keyword "travel" --url "https://example.com"
  python site_builder.py css --design DESIGN.md
  python site_builder.py parse-design DESIGN.md
        """)
        return

    command = sys.argv[1]

    if command == "scaffold":
        project_dir = sys.argv[2] if len(sys.argv) > 2 else "./site"
        name = "My Website"
        keyword = "website"
        url = "https://example.com"
        design = None

        for i, arg in enumerate(sys.argv):
            if arg == "--name" and i + 1 < len(sys.argv):
                name = sys.argv[i + 1]
            if arg == "--keyword" and i + 1 < len(sys.argv):
                keyword = sys.argv[i + 1]
            if arg == "--url" and i + 1 < len(sys.argv):
                url = sys.argv[i + 1]
            if arg == "--design" and i + 1 < len(sys.argv):
                design = sys.argv[i + 1]

        print(f"🏗️ Scaffolding project at {project_dir}...")
        result = scaffold_project(project_dir, name, keyword, url, design_md_path=design)
        print(f"✅ Created {len(result['created_files'])} files:")
        for f in result["created_files"]:
            print(f"  📄 {f}")

    elif command == "css":
        design = None
        for i, arg in enumerate(sys.argv):
            if arg == "--design" and i + 1 < len(sys.argv):
                design = sys.argv[i + 1]
        tokens = parse_design_md(design) if design else {}
        print(generate_global_css(tokens))

    elif command == "parse-design":
        path = sys.argv[2] if len(sys.argv) > 2 else "DESIGN.md"
        tokens = parse_design_md(path)
        print(json.dumps(tokens, indent=2))

    else:
        print(f"Unknown command: {command}")


if __name__ == "__main__":
    main()

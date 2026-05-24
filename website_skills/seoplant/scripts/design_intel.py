"""
SEOplant Design Intelligence — Phase 3 of the pipeline.

Searches for design inspiration, extracts design tokens from real websites,
manages a local design library, and fuses multiple designs into a project DESIGN.md.

Dependencies: requests, beautifulsoup4 (pip install requests beautifulsoup4)
"""

import json
import os
import re
import sys
import hashlib
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional
from urllib.parse import quote_plus, urlparse

try:
    import requests
except ImportError:
    print("ERROR: 'requests' package required. Install: pip install requests")
    sys.exit(1)

try:
    from bs4 import BeautifulSoup
except ImportError:
    BeautifulSoup = None  # Optional, graceful degradation

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
JINA_READER = "https://r.jina.ai/"
JINA_SEARCH = "https://s.jina.ai/"
DESIGN_LIBRARY_DIR = Path(__file__).parent.parent / "design-library"
EXTRACTED_DIR = DESIGN_LIBRARY_DIR / "extracted"
INDEX_PATH = DESIGN_LIBRARY_DIR / "index.json"
REQUEST_TIMEOUT = 30
HEADERS = {
    "User-Agent": "Mozilla/5.0 (compatible; DesignIntel/1.0)",
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
}

# ---------------------------------------------------------------------------
# 1. Search Design Inspiration
# ---------------------------------------------------------------------------

def search_design_inspiration(
    keyword: str,
    site_type: str = "website",
    num_results: int = 15,
) -> list[dict]:
    """Search for excellent websites in the same niche for design inspiration.

    Uses Jina Search (s.jina.ai) with multiple queries targeting design galleries
    and general web results.

    Returns:
        List of {url, title, snippet, source, relevance_note}
    """
    queries = [
        f"awwwards {keyword} website",
        f"pageflows {keyword} user flow",
        f"site:godly.website {keyword}",
        f"mobbin {keyword} web design",
        f"site:land-book.com {keyword}",
        f"designeverywhere.co {keyword}",
        f"siteinspire {keyword}",
        f"{keyword} lapa.ninja landing page",
        f"best {site_type} website design 2025 {keyword}",
    ]

    all_results = []
    seen_urls = set()

    for query in queries:
        try:
            resp = requests.get(
                JINA_SEARCH + quote_plus(query),
                headers={**HEADERS, "Accept": "application/json"},
                timeout=REQUEST_TIMEOUT,
            )
            if resp.status_code == 200:
                # Jina search returns markdown, parse URLs from it
                text = resp.text
                # Extract URLs from markdown links [title](url)
                links = re.findall(r'\[([^\]]+)\]\((https?://[^\)]+)\)', text)
                for title, url in links:
                    parsed = urlparse(url)
                    # Skip search engines, social media, and non-website URLs
                    skip_domains = ['google.com', 'youtube.com', 'facebook.com',
                                    'twitter.com', 'reddit.com', 'pinterest.com',
                                    'jina.ai', 'bing.com']
                    if any(d in parsed.netloc for d in skip_domains):
                        continue
                    if url not in seen_urls:
                        seen_urls.add(url)
                        all_results.append({
                            "url": url,
                            "title": title.strip(),
                            "snippet": "",
                            "source": f"jina_search:{query[:40]}",
                            "relevance_note": _classify_design_source(url),
                        })
        except Exception as e:
            print(f"  [WARN] Search failed for '{query[:40]}': {e}")
            continue

    # Deduplicate by domain
    domain_seen = set()
    unique_results = []
    for r in all_results:
        domain = urlparse(r["url"]).netloc
        if domain not in domain_seen:
            domain_seen.add(domain)
            unique_results.append(r)

    return unique_results[:num_results]


def _classify_design_source(url: str) -> str:
    """Classify URL source for relevance scoring."""
    domain = urlparse(url).netloc.lower()
    if "awwwards" in domain:
        return "award-winning design gallery"
    elif "siteinspire" in domain:
        return "curated design inspiration"
    elif "godly" in domain:
        return "curated modern web design"
    elif "land-book" in domain:
        return "landing page gallery (color filters)"
    elif "mobbin" in domain:
        return "mobile + web UI patterns"
    elif "pageflows" in domain:
        return "real user flow screenshots"
    elif "designeverywhere" in domain:
        return "global design showcase"
    elif "lapa.ninja" in domain:
        return "landing page gallery"
    elif "dribbble" in domain:
        return "designer portfolio"
    elif "behance" in domain:
        return "design showcase"
    else:
        return "direct website"


# ---------------------------------------------------------------------------
# 2. Extract Design Tokens
# ---------------------------------------------------------------------------

def extract_design_tokens(url: str) -> dict:
    """Extract design tokens (colors, fonts, layout) from a URL.

    Tries direct HTML fetch first (best for inline CSS), then falls back to
    Jina Reader with markdown-aware extraction, and finally tries fetching
    external CSS stylesheets linked in the HTML.

    Returns:
        Dict with keys: url, title, colors, typography, layout, components, raw_excerpt
    """
    result = {
        "url": url,
        "title": "",
        "extracted_at": datetime.now(timezone.utc).isoformat(),
        "colors": {"primary": [], "all_hex": [], "gradients": []},
        "typography": {"fonts": [], "font_stacks": []},
        "layout": {"patterns": [], "max_width": None},
        "components": [],
        "raw_excerpt": "",
    }

    html_content = ""

    # Method B (now first): Direct fetch — raw HTML has the best chance of
    # containing inline styles, <style> blocks, and <link> tags for CSS.
    try:
        resp = requests.get(url, headers=HEADERS, timeout=REQUEST_TIMEOUT)
        if resp.status_code == 200:
            html_content = resp.text
            result["raw_excerpt"] = html_content[:2000]
    except Exception as e:
        print(f"  [WARN] Direct fetch failed for {url}: {e}")

    # Method A (now fallback): Jina Reader — returns Markdown, so we use a
    # secondary markdown-aware extraction pass instead of CSS regex.
    jina_markdown = ""
    if not html_content:
        try:
            resp = requests.get(
                JINA_READER + url,
                headers={**HEADERS, "Accept": "text/html"},
                timeout=REQUEST_TIMEOUT,
            )
            if resp.status_code == 200:
                jina_markdown = resp.text
                result["raw_excerpt"] = jina_markdown[:2000]
        except Exception as e:
            print(f"  [WARN] Jina Reader also failed for {url}: {e}")

    # If we only have Jina markdown (no raw HTML), do markdown-aware extraction
    if jina_markdown and not html_content:
        result["colors"] = _extract_colors_from_markdown(jina_markdown)
        result["typography"] = _extract_typography_from_markdown(jina_markdown)
        # Title: first markdown heading
        md_title = re.search(r'^#\s+(.+)', jina_markdown, re.M)
        if md_title:
            result["title"] = md_title.group(1).strip()
        return result

    if not html_content:
        return result

    # --- We have raw HTML — proceed with standard extraction ---

    # Extract title
    title_match = re.search(r'<title[^>]*>([^<]+)</title>', html_content, re.I)
    if title_match:
        result["title"] = title_match.group(1).strip()

    # Method C: Fetch external CSS stylesheets linked in the HTML.
    # Most real design tokens live in external .css files, not inline.
    css_content = _fetch_external_css(html_content, url)
    combined_content = html_content + "\n" + css_content

    # Extract colors (from HTML + external CSS)
    result["colors"] = _extract_colors(combined_content)

    # Extract typography (from HTML + external CSS)
    result["typography"] = _extract_typography(combined_content)

    # Extract layout patterns (from HTML + external CSS)
    result["layout"] = _extract_layout(combined_content)

    # Extract component patterns (HTML only — needs markup)
    result["components"] = _extract_components(html_content)

    return result


def _extract_colors(html: str) -> dict:
    """Extract color information from HTML/CSS content."""
    colors = {"primary": [], "all_hex": [], "gradients": [], "css_vars": {}}

    # Find hex colors
    hex_colors = re.findall(r'#(?:[0-9a-fA-F]{3}){1,2}\b', html)
    # Normalize to 6-char hex
    normalized = set()
    for c in hex_colors:
        if len(c) == 4:  # #abc -> #aabbcc
            c = f"#{c[1]*2}{c[2]*2}{c[3]*2}"
        normalized.add(c.lower())

    # Filter out common non-design colors (pure black/white, GitHub UI colors, etc.)
    skip_colors = {'#000000', '#ffffff', '#000', '#fff', '#333333', '#666666',
                   '#999999', '#cccccc', '#f5f5f5', '#e5e5e5', '#1e2327'}
    design_colors = [c for c in normalized if c not in skip_colors]

    colors["all_hex"] = sorted(design_colors)[:20]  # Top 20 unique colors
    colors["primary"] = colors["all_hex"][:5]  # First 5 as primary palette

    # Find CSS custom properties for colors
    css_var_matches = re.findall(
        r'--([\w-]*(?:color|bg|text|primary|secondary|accent)[\w-]*):\s*([^;]+)',
        html, re.I
    )
    for name, value in css_var_matches[:15]:
        colors["css_vars"][f"--{name}"] = value.strip()

    # Find gradients
    gradient_matches = re.findall(
        r'(?:linear|radial|conic)-gradient\([^)]+\)', html, re.I
    )
    colors["gradients"] = list(set(gradient_matches))[:5]

    # Find rgb/rgba/hsl colors
    rgb_matches = re.findall(r'rgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+(?:\s*,\s*[\d.]+)?\s*\)', html)
    if rgb_matches:
        colors["rgb_colors"] = list(set(rgb_matches))[:10]

    return colors


def _extract_colors_from_markdown(md_text: str) -> dict:
    """Extract color info from Markdown text (Jina Reader output).

    Looks for hex codes and named color references that appear in
    design documentation, color swatches, etc.
    """
    colors = {"primary": [], "all_hex": [], "gradients": [], "css_vars": {}}

    # Hex colors that appear anywhere in markdown text
    hex_colors = re.findall(r'#(?:[0-9a-fA-F]{3}){1,2}\b', md_text)
    normalized = set()
    for c in hex_colors:
        if len(c) == 4:
            c = f"#{c[1]*2}{c[2]*2}{c[3]*2}"
        normalized.add(c.lower())

    skip_colors = {'#000000', '#ffffff', '#000', '#fff', '#333333', '#666666',
                   '#999999', '#cccccc', '#f5f5f5', '#e5e5e5', '#1e2327'}
    design_colors = [c for c in normalized if c not in skip_colors]

    colors["all_hex"] = sorted(design_colors)[:20]
    colors["primary"] = colors["all_hex"][:5]

    return colors


def _extract_typography_from_markdown(md_text: str) -> dict:
    """Extract typography hints from Markdown text (Jina Reader output).

    Looks for well-known web font names mentioned in text.
    """
    typo = {"fonts": [], "font_stacks": [], "google_fonts": [], "font_sizes": []}

    well_known_fonts = [
        'Inter', 'Roboto', 'Poppins', 'Montserrat', 'Open Sans', 'Lato',
        'Raleway', 'Nunito', 'Playfair Display', 'Merriweather', 'Source Sans',
        'Work Sans', 'DM Sans', 'Space Grotesk', 'Plus Jakarta Sans',
        'Manrope', 'Outfit', 'Sora', 'Figtree', 'Geist', 'Satoshi',
        'General Sans', 'Cabinet Grotesk', 'Clash Display', 'Switzer',
        'Onest', 'Instrument Sans', 'Urbanist', 'Albert Sans',
        'Bricolage Grotesque', 'Lexend', 'Red Hat Display',
        'IBM Plex Sans', 'IBM Plex Mono', 'Fira Code', 'JetBrains Mono',
        'PT Sans', 'PT Serif', 'Noto Sans', 'Noto Serif', 'Ubuntu',
        'Rubik', 'Karla', 'Quicksand', 'Josefin Sans', 'Barlow',
        'Libre Baskerville', 'Crimson Text', 'Cormorant Garamond',
    ]

    found = []
    text_lower = md_text.lower()
    for font in well_known_fonts:
        if font.lower() in text_lower:
            found.append(font)

    typo["fonts"] = found[:10]
    typo["google_fonts"] = found[:3]

    return typo


def _fetch_external_css(html: str, base_url: str) -> str:
    """Fetch external CSS files linked in the HTML.

    Extracts <link rel="stylesheet" href="..."> URLs, resolves them
    relative to base_url, and fetches the first 3.

    Returns concatenated CSS text.
    """
    css_urls = re.findall(
        r'<link[^>]+rel=["\']stylesheet["\'][^>]+href=["\']([^"\']+)["\']'
        r'|<link[^>]+href=["\']([^"\']+)["\'][^>]+rel=["\']stylesheet["\']',
        html, re.I,
    )
    # Flatten tuples from alternation groups and remove empties
    flat_urls = [u for pair in css_urls for u in pair if u]

    parsed_base = urlparse(base_url)
    base_origin = f"{parsed_base.scheme}://{parsed_base.netloc}"

    resolved = []
    for href in flat_urls:
        if href.startswith('//'):
            href = parsed_base.scheme + ':' + href
        elif href.startswith('/'):
            href = base_origin + href
        elif not href.startswith('http'):
            # Relative path
            base_path = base_url.rsplit('/', 1)[0]
            href = base_path + '/' + href
        resolved.append(href)

    css_parts = []
    for css_url in resolved[:3]:  # Limit to first 3 stylesheets
        try:
            resp = requests.get(
                css_url,
                headers={**HEADERS, "Accept": "text/css,*/*"},
                timeout=REQUEST_TIMEOUT,
            )
            if resp.status_code == 200 and len(resp.text) < 500_000:
                css_parts.append(resp.text)
        except Exception as e:
            print(f"  [WARN] CSS fetch failed for {css_url}: {e}")

    return "\n".join(css_parts)


def _extract_typography(html: str) -> dict:
    """Extract typography information from HTML/CSS content."""
    typo = {"fonts": [], "font_stacks": [], "google_fonts": [], "font_sizes": []}

    # Find font-family declarations
    font_matches = re.findall(r'font-family:\s*([^;}{]+)', html, re.I)
    all_fonts = set()
    for fm in font_matches:
        # Clean up and split font stacks
        fonts = [f.strip().strip("'\"") for f in fm.split(',')]
        all_fonts.update(fonts)
        typo["font_stacks"].append(fm.strip())

    # Filter out generic font families
    generic = {'serif', 'sans-serif', 'monospace', 'cursive', 'fantasy',
               'system-ui', 'ui-sans-serif', 'ui-serif', 'ui-monospace',
               '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'inherit'}
    design_fonts = [f for f in all_fonts if f not in generic and len(f) > 1]
    typo["fonts"] = sorted(set(design_fonts))[:10]

    # Find Google Fonts imports
    gf_matches = re.findall(r'fonts\.googleapis\.com/css2?\?family=([^"&\s]+)', html)
    for gf in gf_matches:
        font_name = gf.replace('+', ' ').split(':')[0]
        typo["google_fonts"].append(font_name)

    # Find font-size declarations
    size_matches = re.findall(r'font-size:\s*([\d.]+(?:px|rem|em|vw))', html, re.I)
    typo["font_sizes"] = sorted(set(size_matches))[:10]

    # Deduplicate font stacks
    typo["font_stacks"] = list(set(typo["font_stacks"]))[:5]

    return typo


def _extract_layout(html: str) -> dict:
    """Extract layout patterns from HTML/CSS content."""
    layout = {"patterns": [], "max_width": None, "spacing": []}

    # Detect layout patterns
    if re.search(r'display:\s*grid', html, re.I):
        layout["patterns"].append("css-grid")
    if re.search(r'display:\s*flex', html, re.I):
        layout["patterns"].append("flexbox")
    if re.search(r'container', html, re.I):
        layout["patterns"].append("container")

    # Find max-width values (likely content width)
    max_widths = re.findall(r'max-width:\s*([\d]+(?:px|rem|em))', html, re.I)
    if max_widths:
        layout["max_width"] = max_widths[0]

    # Find spacing/gap values
    gap_matches = re.findall(r'(?:gap|padding|margin):\s*([\d.]+(?:px|rem|em))', html, re.I)
    layout["spacing"] = sorted(set(gap_matches))[:8]

    return layout


def _extract_components(html: str) -> list[str]:
    """Identify common UI component patterns in HTML."""
    components = []

    patterns = {
        "navigation": r'<nav|role="navigation"|class="[^"]*nav[^"]*"',
        "hero": r'class="[^"]*hero[^"]*"|class="[^"]*banner[^"]*"',
        "card": r'class="[^"]*card[^"]*"',
        "footer": r'<footer|class="[^"]*footer[^"]*"',
        "sidebar": r'class="[^"]*sidebar[^"]*"|<aside',
        "modal": r'class="[^"]*modal[^"]*"|class="[^"]*dialog[^"]*"',
        "carousel": r'class="[^"]*carousel[^"]*"|class="[^"]*slider[^"]*"|class="[^"]*swiper[^"]*"',
        "accordion": r'class="[^"]*accordion[^"]*"',
        "tabs": r'class="[^"]*tabs?[^"]*"|role="tablist"',
        "breadcrumb": r'class="[^"]*breadcrumb[^"]*"',
        "cta": r'class="[^"]*cta[^"]*"|class="[^"]*call-to-action[^"]*"',
        "testimonial": r'class="[^"]*testimonial[^"]*"|class="[^"]*review[^"]*"',
        "pricing": r'class="[^"]*pricing[^"]*"',
        "feature-grid": r'class="[^"]*features?[^"]*"',
    }

    for component, pattern in patterns.items():
        if re.search(pattern, html, re.I):
            components.append(component)

    return components


# ---------------------------------------------------------------------------
# 3. Generate DESIGN.md
# ---------------------------------------------------------------------------

def generate_design_md(tokens: dict, site_name: str = "Untitled") -> str:
    """Generate a DESIGN.md file from extracted design tokens.

    Follows the standard DESIGN.md format with YAML frontmatter
    and markdown body.
    """
    colors = tokens.get("colors", {})
    typo = tokens.get("typography", {})
    layout = tokens.get("layout", {})
    components = tokens.get("components", [])

    # Build YAML frontmatter
    primary_colors = colors.get("primary", [])
    fonts = typo.get("fonts", []) or typo.get("google_fonts", [])

    md = f"""---
name: "{site_name}"
source_url: "{tokens.get('url', '')}"
extracted_at: "{tokens.get('extracted_at', datetime.now(timezone.utc).isoformat())}"
tokens:
  colors:
    primary: "{primary_colors[0] if primary_colors else '#3B82F6'}"
    secondary: "{primary_colors[1] if len(primary_colors) > 1 else '#10B981'}"
    accent: "{primary_colors[2] if len(primary_colors) > 2 else '#F59E0B'}"
    background: "#ffffff"
    surface: "#f8fafc"
    text: "#1e293b"
    text-muted: "#64748b"
  typography:
    heading: "{fonts[0] if fonts else 'Inter'}"
    body: "{fonts[1] if len(fonts) > 1 else fonts[0] if fonts else 'Inter'}"
    mono: "JetBrains Mono"
  spacing:
    base: "4px"
    scale: [4, 8, 12, 16, 24, 32, 48, 64, 96]
---

# {site_name} — Design System

## Color Palette

| Role | Value | Usage |
|------|-------|-------|
| Primary | `{primary_colors[0] if primary_colors else '#3B82F6'}` | Brand color, CTAs, links |
| Secondary | `{primary_colors[1] if len(primary_colors) > 1 else '#10B981'}` | Supporting elements |
| Accent | `{primary_colors[2] if len(primary_colors) > 2 else '#F59E0B'}` | Highlights, badges |
| Background | `#ffffff` | Page background |
| Surface | `#f8fafc` | Card/section backgrounds |
| Text | `#1e293b` | Body text |
| Text Muted | `#64748b` | Secondary text |
"""

    if colors.get("all_hex"):
        md += "\n### Full Palette\n"
        md += "```\n" + " ".join(colors["all_hex"][:12]) + "\n```\n"

    if colors.get("css_vars"):
        md += "\n### CSS Variables\n```css\n"
        for var, val in list(colors["css_vars"].items())[:10]:
            md += f"  {var}: {val};\n"
        md += "```\n"

    if colors.get("gradients"):
        md += "\n### Gradients\n```css\n"
        for g in colors["gradients"][:3]:
            md += f"  background: {g};\n"
        md += "```\n"

    # Typography
    md += f"""
## Typography

| Role | Font | Fallback |
|------|------|----------|
| Headings | {fonts[0] if fonts else 'Inter'} | system-ui, sans-serif |
| Body | {fonts[1] if len(fonts) > 1 else fonts[0] if fonts else 'Inter'} | system-ui, sans-serif |
| Monospace | JetBrains Mono | ui-monospace, monospace |
"""

    if typo.get("google_fonts"):
        md += "\n### Google Fonts Import\n```html\n"
        font_params = "|".join([f"family={f.replace(' ', '+')}" for f in typo["google_fonts"][:3]])
        md += f'<link href="https://fonts.googleapis.com/css2?{font_params}&display=swap" rel="stylesheet">\n'
        md += "```\n"

    if typo.get("font_sizes"):
        md += "\n### Type Scale\n"
        md += "```\n" + " → ".join(typo["font_sizes"][:8]) + "\n```\n"

    # Layout
    md += "\n## Layout\n\n"
    if layout.get("patterns"):
        md += f"- **Patterns**: {', '.join(layout['patterns'])}\n"
    if layout.get("max_width"):
        md += f"- **Max Width**: {layout['max_width']}\n"
    if layout.get("spacing"):
        md += f"- **Common Spacing**: {', '.join(layout['spacing'][:5])}\n"

    # Components
    if components:
        md += "\n## Components Detected\n\n"
        for comp in components:
            md += f"- ✅ {comp.replace('-', ' ').title()}\n"

    # Design philosophy (generic, to be refined by agent)
    md += """
## Design Philosophy

> This design system was auto-extracted and should be reviewed and refined.
> The orchestrating AI agent will customize these tokens based on:
> - Brand positioning from SEO analysis
> - User preferences from design selection
> - Site type requirements

### Principles
1. **Clarity** — Content-first, minimal visual noise
2. **Consistency** — Systematic spacing and typography
3. **Accessibility** — WCAG 2.1 AA contrast ratios
4. **Performance** — Minimal custom fonts, optimized assets
"""

    return md


# ---------------------------------------------------------------------------
# 4. Design Library Management
# ---------------------------------------------------------------------------

def manage_design_library(
    action: str,
    design_data: Optional[dict] = None,
    search_query: str = "",
) -> dict:
    """Manage the local design reference library.

    Args:
        action: 'add', 'list', 'search', 'get'
        design_data: Design tokens dict (for 'add')
        search_query: Search string (for 'search')

    Returns:
        Operation result dict
    """
    # Ensure directories exist
    DESIGN_LIBRARY_DIR.mkdir(parents=True, exist_ok=True)
    EXTRACTED_DIR.mkdir(parents=True, exist_ok=True)

    # Load or create index
    index = []
    if INDEX_PATH.exists():
        try:
            index = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            index = []

    if action == "add" and design_data:
        # Generate a unique ID
        url = design_data.get("url", "unknown")
        design_id = hashlib.md5(url.encode()).hexdigest()[:12]
        title = design_data.get("title", "Untitled")

        # Save design tokens as JSON
        json_path = EXTRACTED_DIR / f"{design_id}.json"
        json_path.write_text(
            json.dumps(design_data, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

        # Save DESIGN.md
        md_path = EXTRACTED_DIR / f"{design_id}.md"
        md_content = generate_design_md(design_data, title)
        md_path.write_text(md_content, encoding="utf-8")

        # Update index
        entry = {
            "id": design_id,
            "title": title,
            "url": url,
            "colors": design_data.get("colors", {}).get("primary", [])[:3],
            "fonts": design_data.get("typography", {}).get("fonts", [])[:2],
            "components": design_data.get("components", []),
            "extracted_at": design_data.get("extracted_at", ""),
            "json_file": str(json_path.name),
            "md_file": str(md_path.name),
        }

        # Update or append
        existing_ids = {e["id"] for e in index}
        if design_id in existing_ids:
            index = [entry if e["id"] == design_id else e for e in index]
        else:
            index.append(entry)

        INDEX_PATH.write_text(
            json.dumps(index, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )

        return {"status": "added", "id": design_id, "files": [str(json_path), str(md_path)]}

    elif action == "list":
        return {"status": "ok", "count": len(index), "designs": index}

    elif action == "search":
        query_lower = search_query.lower()
        matches = [
            e for e in index
            if query_lower in e.get("title", "").lower()
            or query_lower in e.get("url", "").lower()
            or any(query_lower in c for c in e.get("components", []))
        ]
        return {"status": "ok", "count": len(matches), "designs": matches}

    elif action == "get" and search_query:
        match = next((e for e in index if e["id"] == search_query), None)
        if match:
            json_path = EXTRACTED_DIR / match["json_file"]
            if json_path.exists():
                data = json.loads(json_path.read_text(encoding="utf-8"))
                return {"status": "ok", "design": data}
        return {"status": "not_found"}

    return {"status": "error", "message": f"Unknown action: {action}"}


# ---------------------------------------------------------------------------
# 5. Fuse Designs
# ---------------------------------------------------------------------------

def fuse_designs(
    selected_designs: list[dict],
    brand_keywords: list[str],
    site_type: str = "website",
) -> str:
    """Fuse 1-3 selected design token sets into a unified project DESIGN.md.

    Harmonizes color palettes, picks best typography, merges components.
    """
    if not selected_designs:
        # Generate default design
        return generate_design_md({
            "url": "generated",
            "colors": {"primary": ["#3B82F6", "#10B981", "#F59E0B"]},
            "typography": {"fonts": ["Inter"], "google_fonts": ["Inter"]},
            "layout": {"patterns": ["flexbox", "css-grid"]},
            "components": ["navigation", "hero", "card", "footer"],
        }, f"{' '.join(brand_keywords).title()} Website")

    # Collect all colors from selected designs
    all_colors = []
    all_fonts = []
    all_components = set()
    all_gradients = []
    all_css_vars = {}

    for design in selected_designs:
        colors = design.get("colors", {})
        all_colors.extend(colors.get("primary", []))
        all_colors.extend(colors.get("all_hex", []))
        all_gradients.extend(colors.get("gradients", []))
        all_css_vars.update(colors.get("css_vars", {}))

        typo = design.get("typography", {})
        all_fonts.extend(typo.get("fonts", []))
        all_fonts.extend(typo.get("google_fonts", []))

        all_components.update(design.get("components", []))

    # Deduplicate and prioritize
    # Colors: take first occurrence as primary, then unique others
    seen_colors = []
    for c in all_colors:
        if c not in seen_colors:
            seen_colors.append(c)

    # Fonts: deduplicate, keep order
    seen_fonts = []
    for f in all_fonts:
        if f not in seen_fonts and f:
            seen_fonts.append(f)

    # Build fused tokens
    fused = {
        "url": "fused-design",
        "title": f"{' '.join(brand_keywords).title()} — {site_type.title()}",
        "extracted_at": datetime.now(timezone.utc).isoformat(),
        "colors": {
            "primary": seen_colors[:5],
            "all_hex": seen_colors[:15],
            "gradients": list(set(all_gradients))[:3],
            "css_vars": dict(list(all_css_vars.items())[:10]),
        },
        "typography": {
            "fonts": seen_fonts[:4],
            "google_fonts": seen_fonts[:3],
            "font_stacks": [],
        },
        "layout": {
            "patterns": ["flexbox", "css-grid", "container"],
            "max_width": "1280px",
        },
        "components": sorted(all_components),
    }

    site_name = f"{' '.join(brand_keywords).title()} — {site_type.title()}"
    return generate_design_md(fused, site_name)


# ---------------------------------------------------------------------------
# 6. Present Designs
# ---------------------------------------------------------------------------

def present_designs(designs: list[dict]) -> str:
    """Format extracted designs for user presentation.

    Returns Markdown showing each design's key characteristics.
    """
    if not designs:
        return "No designs to present."

    lines = ["# 🎨 Design Inspiration Options\n"]
    lines.append("Review the following designs and tell me which ones you like (e.g., '1, 3, 5').\n")

    for i, design in enumerate(designs, 1):
        title = design.get("title", "Untitled")
        url = design.get("url", "N/A")
        colors = design.get("colors", {})
        typo = design.get("typography", {})
        components = design.get("components", [])

        primary = colors.get("primary", [])
        fonts = typo.get("fonts", []) or typo.get("google_fonts", [])

        lines.append(f"## Option {i}: {title}")
        lines.append(f"**URL**: [{url}]({url})\n")

        # Color palette
        if primary:
            palette_str = " ".join([f"`{c}`" for c in primary[:5]])
            lines.append(f"**Colors**: {palette_str}\n")

        # Fonts
        if fonts:
            lines.append(f"**Fonts**: {', '.join(fonts[:3])}\n")

        # Components
        if components:
            comp_str = ", ".join(components[:6])
            lines.append(f"**Components**: {comp_str}\n")

        # Gradients
        if colors.get("gradients"):
            lines.append(f"**Gradients**: ✅ Uses gradient effects\n")

        lines.append("---\n")

    lines.append("\n> 💡 Tell me which numbers you like, and I'll fuse them into your project's DESIGN.md.")

    return "\n".join(lines)


# ---------------------------------------------------------------------------
# CLI Interface
# ---------------------------------------------------------------------------

def main():
    """CLI entry point."""
    # Fix Windows GBK encoding crash when printing emoji / Unicode
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')

    if len(sys.argv) < 2:
        print("""
Usage:
  python design_intel.py search "travel website"
  python design_intel.py extract "https://example.com"
  python design_intel.py generate "https://example.com" "My Site"
  python design_intel.py library list
  python design_intel.py library search "travel"
  python design_intel.py present
  python design_intel.py fuse --keywords "travel scotland" --type "travel"
        """)
        return

    command = sys.argv[1]

    if command == "search":
        keyword = sys.argv[2] if len(sys.argv) > 2 else "website"
        site_type = sys.argv[3] if len(sys.argv) > 3 else "website"
        print(f"🔍 Searching design inspiration for '{keyword}'...")
        results = search_design_inspiration(keyword, site_type)
        print(f"\nFound {len(results)} design references:\n")
        for i, r in enumerate(results, 1):
            print(f"  {i}. {r['title'][:60]}")
            print(f"     {r['url']}")
            print(f"     Source: {r['relevance_note']}")
            print()

    elif command == "extract":
        url = sys.argv[2] if len(sys.argv) > 2 else ""
        if not url:
            print("ERROR: URL required")
            return
        print(f"🎨 Extracting design tokens from {url}...")
        tokens = extract_design_tokens(url)
        print(json.dumps(tokens, indent=2, ensure_ascii=False, default=str))

    elif command == "generate":
        url = sys.argv[2] if len(sys.argv) > 2 else ""
        name = sys.argv[3] if len(sys.argv) > 3 else "My Website"
        if not url:
            print("ERROR: URL required")
            return
        print(f"📄 Generating DESIGN.md from {url}...")
        tokens = extract_design_tokens(url)
        md = generate_design_md(tokens, name)
        print(md)

    elif command == "library":
        sub = sys.argv[2] if len(sys.argv) > 2 else "list"
        if sub == "list":
            result = manage_design_library("list")
            print(f"📚 Design Library: {result['count']} designs")
            for d in result["designs"]:
                print(f"  - [{d['id']}] {d['title']} ({d['url'][:50]})")
        elif sub == "search":
            query = sys.argv[3] if len(sys.argv) > 3 else ""
            result = manage_design_library("search", search_query=query)
            print(f"🔍 Found {result['count']} matches for '{query}'")
            for d in result["designs"]:
                print(f"  - [{d['id']}] {d['title']}")

    elif command == "present":
        result = manage_design_library("list")
        if result["designs"]:
            # Load full data for each
            full_designs = []
            for entry in result["designs"]:
                r = manage_design_library("get", search_query=entry["id"])
                if r["status"] == "ok":
                    full_designs.append(r["design"])
            print(present_designs(full_designs))
        else:
            print("No designs in library. Run 'extract' first.")

    elif command == "fuse":
        print("🔀 Fusing designs from library...")
        keywords = []
        site_type = "website"
        for i, arg in enumerate(sys.argv):
            if arg == "--keywords" and i + 1 < len(sys.argv):
                keywords = sys.argv[i + 1].split()
            if arg == "--type" and i + 1 < len(sys.argv):
                site_type = sys.argv[i + 1]

        result = manage_design_library("list")
        designs = []
        for entry in result.get("designs", []):
            r = manage_design_library("get", search_query=entry["id"])
            if r["status"] == "ok":
                designs.append(r["design"])

        if designs:
            md = fuse_designs(designs, keywords or ["my", "website"], site_type)
            print(md)
        else:
            print("No designs in library to fuse.")

    else:
        print(f"Unknown command: {command}")


if __name__ == "__main__":
    main()

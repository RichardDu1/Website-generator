"""
SEOplant Competitor Intelligence — Phase 1 of the pipeline.

Searches for competitors, crawls their websites, analyzes SEO signals,
checks social media presence, and generates a comprehensive report.

Dependencies: requests (pip install requests)
"""

import json
import re
import sys
from datetime import datetime, timezone
from typing import Optional
from urllib.parse import quote_plus, urlparse

try:
    import requests
except ImportError:
    print("ERROR: 'requests' package required. Install: pip install requests")
    sys.exit(1)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
JINA_READER = "https://r.jina.ai/"
JINA_SEARCH = "https://s.jina.ai/"
REQUEST_TIMEOUT = 30
HEADERS = {
    "User-Agent": "Mozilla/5.0 (compatible; CompetitorIntel/1.0)",
    "Accept": "text/html,application/xhtml+xml,*/*;q=0.8",
}

# ---------------------------------------------------------------------------
# 1. Search Competitors
# ---------------------------------------------------------------------------

def search_competitors(
    keyword: str,
    site_type: str = "website",
    num_results: int = 15,
) -> list[dict]:
    """Search for competitor websites in the same niche.

    Uses Jina Search with multiple query patterns.

    Returns:
        List of {url, title, snippet, source}
    """
    queries = [
        f"best {keyword} {site_type} 2025",
        f"top {keyword} websites",
        f"{keyword} {site_type} examples",
        f"{keyword} competitors comparison",
    ]

    all_results = []
    seen_domains = set()

    for query in queries:
        try:
            resp = requests.get(
                JINA_SEARCH + quote_plus(query),
                headers={**HEADERS, "Accept": "application/json"},
                timeout=REQUEST_TIMEOUT,
            )
            if resp.status_code == 200:
                text = resp.text
                links = re.findall(r'\[([^\]]+)\]\((https?://[^\)]+)\)', text)
                for title, url in links:
                    domain = urlparse(url).netloc
                    skip = ['google.', 'youtube.', 'facebook.', 'twitter.',
                            'reddit.', 'wikipedia.', 'jina.ai', 'bing.',
                            'amazon.', 'pinterest.']
                    if any(s in domain for s in skip):
                        continue
                    if domain not in seen_domains:
                        seen_domains.add(domain)
                        all_results.append({
                            "url": url,
                            "title": title.strip(),
                            "snippet": "",
                            "source": query[:50],
                            "domain": domain,
                        })
        except Exception as e:
            print(f"  [WARN] Search failed for '{query[:40]}': {e}")
            continue

    return all_results[:num_results]


# ---------------------------------------------------------------------------
# 2. Crawl Website
# ---------------------------------------------------------------------------

def crawl_website(url: str) -> dict:
    """Fetch and analyze a website's structure and content.

    Uses Jina Reader for content extraction with direct fetch fallback.

    Returns:
        Dict with title, meta, headings, word_count, links, tech_hints
    """
    result = {
        "url": url,
        "domain": urlparse(url).netloc,
        "crawled_at": datetime.now(timezone.utc).isoformat(),
        "title": "",
        "meta_description": "",
        "h1_tags": [],
        "h2_tags": [],
        "h3_tags": [],
        "word_count": 0,
        "internal_links": 0,
        "external_links": 0,
        "images_total": 0,
        "images_with_alt": 0,
        "tech_hints": [],
        "schema_types": [],
        "has_sitemap": False,
        "has_robots_txt": False,
        "content_excerpt": "",
        "error": None,
    }

    # Fetch with Jina Reader
    content = ""
    try:
        resp = requests.get(
            JINA_READER + url,
            headers=HEADERS,
            timeout=REQUEST_TIMEOUT,
        )
        if resp.status_code == 200:
            content = resp.text
    except Exception as e:
        # Fallback to direct fetch
        try:
            resp = requests.get(url, headers=HEADERS, timeout=REQUEST_TIMEOUT)
            content = resp.text if resp.status_code == 200 else ""
        except Exception as e2:
            result["error"] = str(e2)
            return result

    if not content:
        result["error"] = "No content retrieved"
        return result

    # Extract title
    title_match = re.search(r'<title[^>]*>([^<]+)</title>', content, re.I)
    if title_match:
        result["title"] = title_match.group(1).strip()
    elif content.startswith("Title:"):
        result["title"] = content.split("\n")[0].replace("Title:", "").strip()

    # Extract meta description
    meta_match = re.search(
        r'<meta[^>]*name=["\']description["\'][^>]*content=["\']([^"\']+)', content, re.I
    )
    if meta_match:
        result["meta_description"] = meta_match.group(1).strip()

    # Extract headings
    for tag, key in [("h1", "h1_tags"), ("h2", "h2_tags"), ("h3", "h3_tags")]:
        matches = re.findall(f'<{tag}[^>]*>([^<]+)</{tag}>', content, re.I)
        result[key] = [m.strip() for m in matches][:10]

    # Also extract from Jina markdown format (# Heading)
    if not result["h1_tags"]:
        h1_md = re.findall(r'^# (.+)$', content, re.M)
        result["h1_tags"] = h1_md[:3]
    if not result["h2_tags"]:
        h2_md = re.findall(r'^## (.+)$', content, re.M)
        result["h2_tags"] = h2_md[:10]

    # Word count (rough)
    text_only = re.sub(r'<[^>]+>', ' ', content)
    text_only = re.sub(r'\s+', ' ', text_only)
    result["word_count"] = len(text_only.split())

    # Count links
    domain = urlparse(url).netloc
    all_links = re.findall(r'href=["\']([^"\']+)', content, re.I)
    for link in all_links:
        parsed = urlparse(link)
        if parsed.netloc and parsed.netloc != domain:
            result["external_links"] += 1
        else:
            result["internal_links"] += 1

    # Count images
    img_tags = re.findall(r'<img[^>]*>', content, re.I)
    result["images_total"] = len(img_tags)
    result["images_with_alt"] = len([i for i in img_tags if 'alt=' in i.lower()])

    # Detect tech stack
    tech_patterns = {
        "WordPress": r'wp-content|wordpress',
        "React": r'react|__next|_next',
        "Next.js": r'__next|_next/static',
        "Astro": r'astro',
        "Vue": r'vue\.js|nuxt',
        "Tailwind": r'tailwind',
        "Bootstrap": r'bootstrap',
        "jQuery": r'jquery',
        "Shopify": r'cdn\.shopify\.com',
        "Wix": r'wix\.com',
        "Squarespace": r'squarespace',
        "Webflow": r'webflow',
        "Ghost": r'ghost\.io|ghost\.org',
        "Hugo": r'hugo',
        "Gatsby": r'gatsby',
    }
    for tech, pattern in tech_patterns.items():
        if re.search(pattern, content, re.I):
            result["tech_hints"].append(tech)

    # Detect Schema.org
    schema_matches = re.findall(r'"@type"\s*:\s*"([^"]+)"', content)
    result["schema_types"] = list(set(schema_matches))[:10]

    # Content excerpt
    result["content_excerpt"] = text_only[:500].strip()

    # Check robots.txt and sitemap
    base_url = f"{urlparse(url).scheme}://{urlparse(url).netloc}"
    for path, key in [("/robots.txt", "has_robots_txt"), ("/sitemap.xml", "has_sitemap")]:
        try:
            r = requests.head(base_url + path, timeout=5, allow_redirects=True)
            result[key] = r.status_code == 200
        except Exception:
            pass

    return result


# ---------------------------------------------------------------------------
# 3. Analyze SEO Signals
# ---------------------------------------------------------------------------

def analyze_seo_signals(crawl_data: dict) -> dict:
    """Analyze SEO quality of a crawled website.

    Returns:
        Dict with overall_score (1-10), breakdown, and recommendations
    """
    scores = {}
    recommendations = []

    # Title tag
    title = crawl_data.get("title", "")
    title_len = len(title)
    if 30 <= title_len <= 60:
        scores["title"] = 10
    elif 20 <= title_len <= 70:
        scores["title"] = 7
    elif title_len > 0:
        scores["title"] = 4
        recommendations.append(f"Title length ({title_len}) should be 30-60 chars")
    else:
        scores["title"] = 0
        recommendations.append("Missing title tag")

    # Meta description
    desc = crawl_data.get("meta_description", "")
    desc_len = len(desc)
    if 120 <= desc_len <= 160:
        scores["meta_description"] = 10
    elif 80 <= desc_len <= 200:
        scores["meta_description"] = 7
    elif desc_len > 0:
        scores["meta_description"] = 4
        recommendations.append(f"Meta description ({desc_len}) should be 120-160 chars")
    else:
        scores["meta_description"] = 0
        recommendations.append("Missing meta description")

    # H1 tags
    h1s = crawl_data.get("h1_tags", [])
    if len(h1s) == 1:
        scores["h1"] = 10
    elif len(h1s) > 1:
        scores["h1"] = 5
        recommendations.append(f"Multiple H1 tags ({len(h1s)}), should have exactly 1")
    else:
        scores["h1"] = 0
        recommendations.append("Missing H1 tag")

    # Heading hierarchy
    h2s = crawl_data.get("h2_tags", [])
    h3s = crawl_data.get("h3_tags", [])
    if h2s and h1s:
        scores["heading_hierarchy"] = 8 if h3s else 6
    elif h2s:
        scores["heading_hierarchy"] = 4
    else:
        scores["heading_hierarchy"] = 2
        recommendations.append("Weak heading hierarchy, add H2/H3 subheadings")

    # Content depth
    wc = crawl_data.get("word_count", 0)
    if wc >= 2000:
        scores["content_depth"] = 10
    elif wc >= 1000:
        scores["content_depth"] = 7
    elif wc >= 300:
        scores["content_depth"] = 5
    else:
        scores["content_depth"] = 2
        recommendations.append(f"Thin content ({wc} words), aim for 1000+")

    # Image alt text
    total_imgs = crawl_data.get("images_total", 0)
    alt_imgs = crawl_data.get("images_with_alt", 0)
    if total_imgs > 0:
        alt_ratio = alt_imgs / total_imgs
        scores["image_alts"] = round(alt_ratio * 10)
        if alt_ratio < 0.8:
            recommendations.append(f"Only {alt_imgs}/{total_imgs} images have alt text")
    else:
        scores["image_alts"] = 5  # No images, neutral

    # Schema markup
    schemas = crawl_data.get("schema_types", [])
    if len(schemas) >= 3:
        scores["schema"] = 10
    elif len(schemas) >= 1:
        scores["schema"] = 6
    else:
        scores["schema"] = 0
        recommendations.append("No Schema.org structured data found")

    # Technical signals
    tech_score = 5
    if crawl_data.get("has_sitemap"):
        tech_score += 2
    else:
        recommendations.append("No sitemap.xml found")
    if crawl_data.get("has_robots_txt"):
        tech_score += 1
    scores["technical"] = min(tech_score, 10)

    # Internal linking
    internal = crawl_data.get("internal_links", 0)
    if internal >= 20:
        scores["internal_linking"] = 10
    elif internal >= 10:
        scores["internal_linking"] = 7
    elif internal >= 3:
        scores["internal_linking"] = 5
    else:
        scores["internal_linking"] = 2
        recommendations.append("Weak internal linking")

    # Overall score (weighted average)
    weights = {
        "title": 1.5, "meta_description": 1.2, "h1": 1.3,
        "heading_hierarchy": 1.0, "content_depth": 1.5, "image_alts": 0.8,
        "schema": 1.0, "technical": 1.0, "internal_linking": 1.0,
    }
    total_weight = sum(weights.values())
    weighted_sum = sum(scores.get(k, 0) * w for k, w in weights.items())
    overall = round(weighted_sum / total_weight, 1)

    return {
        "url": crawl_data.get("url", ""),
        "overall_score": overall,
        "breakdown": scores,
        "recommendations": recommendations,
        "tech_stack": crawl_data.get("tech_hints", []),
        "schema_types": schemas,
    }


# ---------------------------------------------------------------------------
# 4. Social Media Search
# ---------------------------------------------------------------------------

def search_social_media(competitor_name: str) -> dict:
    """Search for a competitor's social media presence.

    Returns:
        Dict with platforms found and estimated activity level
    """
    result = {
        "competitor": competitor_name,
        "platforms": {},
        "estimated_activity": "unknown",
    }

    platforms = {
        "instagram": f"{competitor_name} instagram",
        "twitter": f"{competitor_name} twitter OR X",
        "facebook": f"{competitor_name} facebook page",
        "linkedin": f"{competitor_name} linkedin",
        "youtube": f"{competitor_name} youtube channel",
        "tiktok": f"{competitor_name} tiktok",
    }

    found_count = 0
    for platform, query in platforms.items():
        try:
            resp = requests.get(
                JINA_SEARCH + quote_plus(query),
                headers=HEADERS,
                timeout=15,
            )
            if resp.status_code == 200:
                text = resp.text.lower()
                domain_map = {
                    "instagram": "instagram.com",
                    "twitter": "twitter.com",
                    "facebook": "facebook.com",
                    "linkedin": "linkedin.com",
                    "youtube": "youtube.com",
                    "tiktok": "tiktok.com",
                }
                if domain_map[platform] in text or f"x.com" in text:
                    result["platforms"][platform] = "found"
                    found_count += 1
                else:
                    result["platforms"][platform] = "not_found"
        except Exception:
            result["platforms"][platform] = "error"

    # Estimate activity
    if found_count >= 4:
        result["estimated_activity"] = "very_active"
    elif found_count >= 2:
        result["estimated_activity"] = "moderate"
    elif found_count >= 1:
        result["estimated_activity"] = "minimal"
    else:
        result["estimated_activity"] = "none_detected"

    return result


# ---------------------------------------------------------------------------
# 5. Generate Competitor Report
# ---------------------------------------------------------------------------

def generate_competitor_report(
    keyword: str,
    competitors: list[dict],
    seo_analyses: list[dict] = None,
    social_data: list[dict] = None,
) -> str:
    """Generate a comprehensive Markdown competitor report.

    Args:
        keyword: The primary keyword/niche
        competitors: List of crawl_website() results
        seo_analyses: List of analyze_seo_signals() results
        social_data: List of search_social_media() results
    """
    seo_analyses = seo_analyses or []
    social_data = social_data or []

    report = f"""# 🔍 Competitor Intelligence Report

**Keyword**: {keyword}
**Date**: {datetime.now().strftime('%Y-%m-%d')}
**Competitors Analyzed**: {len(competitors)}

---

## Executive Summary

"""

    # Build summary table
    report += "| # | Competitor | SEO Score | Tech Stack | Content (words) |\n"
    report += "|---|-----------|-----------|------------|----------------|\n"

    for i, comp in enumerate(competitors):
        seo = seo_analyses[i] if i < len(seo_analyses) else {}
        score = seo.get("overall_score", "N/A")
        tech = ", ".join(comp.get("tech_hints", [])[:3]) or "Unknown"
        wc = comp.get("word_count", 0)
        domain = comp.get("domain", comp.get("url", "N/A"))
        report += f"| {i+1} | {domain} | {score}/10 | {tech} | {wc:,} |\n"

    # SWOT for each competitor
    report += "\n---\n\n## Detailed Analysis\n\n"

    for i, comp in enumerate(competitors):
        seo = seo_analyses[i] if i < len(seo_analyses) else {}
        social = social_data[i] if i < len(social_data) else {}
        domain = comp.get("domain", "Unknown")

        report += f"### {i+1}. {domain}\n\n"
        report += f"**URL**: [{comp.get('url', 'N/A')}]({comp.get('url', '')})\n"
        report += f"**Title**: {comp.get('title', 'N/A')}\n"
        report += f"**Tech**: {', '.join(comp.get('tech_hints', [])) or 'Unknown'}\n\n"

        # SWOT
        strengths = []
        weaknesses = []

        score = seo.get("overall_score", 0)
        if score >= 7:
            strengths.append(f"Strong SEO ({score}/10)")
        elif score <= 4:
            weaknesses.append(f"Weak SEO ({score}/10)")

        if comp.get("word_count", 0) >= 1500:
            strengths.append(f"Deep content ({comp['word_count']:,} words)")
        elif comp.get("word_count", 0) < 500:
            weaknesses.append(f"Thin content ({comp.get('word_count', 0)} words)")

        if comp.get("schema_types"):
            strengths.append(f"Schema: {', '.join(comp['schema_types'][:3])}")
        else:
            weaknesses.append("No structured data")

        if comp.get("has_sitemap"):
            strengths.append("Has sitemap")
        else:
            weaknesses.append("No sitemap")

        # SEO recommendations as weaknesses
        for rec in seo.get("recommendations", [])[:3]:
            weaknesses.append(rec)

        report += "| Strengths ✅ | Weaknesses ❌ |\n"
        report += "|-------------|---------------|\n"
        max_len = max(len(strengths), len(weaknesses))
        for j in range(max_len):
            s = strengths[j] if j < len(strengths) else ""
            w = weaknesses[j] if j < len(weaknesses) else ""
            report += f"| {s} | {w} |\n"

        report += "\n"

    # Gap Analysis
    report += """---

## Gap Analysis & Opportunities

Based on the competitor analysis, here are the key opportunities:

"""
    # Identify common weaknesses
    all_recs = []
    for seo in seo_analyses:
        all_recs.extend(seo.get("recommendations", []))

    if all_recs:
        from collections import Counter
        common = Counter(all_recs).most_common(5)
        report += "### Common Competitor Weaknesses\n\n"
        for rec, count in common:
            pct = round(count / len(seo_analyses) * 100) if seo_analyses else 0
            report += f"- **{rec}** — {count}/{len(seo_analyses)} competitors ({pct}%)\n"
        report += "\n"

    report += """### Strategic Recommendations

1. **Content Gap**: Create content covering topics competitors miss
2. **Technical SEO**: Fix issues competitors haven't addressed
3. **Design Differentiation**: Use Phase 3 Design Intelligence to stand out
4. **Schema Markup**: Implement comprehensive structured data
5. **Multi-language**: If competitors are English-only, add Chinese content

---

> This report was auto-generated. Use it as a foundation for strategic decisions.
"""

    return report


# ---------------------------------------------------------------------------
# CLI Interface
# ---------------------------------------------------------------------------

KNOWN_COMMANDS = {"search", "crawl", "analyze", "social", "report"}


def _usage():
    print("""
Usage:
  python competitor_intel.py search "scottish highlands travel"
  python competitor_intel.py crawl "https://example.com"
  python competitor_intel.py analyze "https://example.com"
  python competitor_intel.py social "Competitor Name"
  python competitor_intel.py report "scottish highlands travel"

If no subcommand is given, the argument is treated as a keyword
and the 'report' command runs automatically. For example:
  python competitor_intel.py "开源 CMS 推荐"
is equivalent to:
  python competitor_intel.py report "开源 CMS 推荐"
    """)


def main():
    # Fix Windows console encoding (GBK) so emoji / CJK chars don't crash
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    sys.stderr.reconfigure(encoding='utf-8', errors='replace')

    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        _usage()
        return

    command = sys.argv[1]

    # --- Smart default: bare keyword → report ---
    if command not in KNOWN_COMMANDS:
        # Treat the first argument as a keyword and run 'report'
        keyword = command
        command = "report"
    else:
        keyword = None  # will be set per-subcommand below

    if command == "search":
        keyword = keyword or (sys.argv[2] if len(sys.argv) > 2 else "travel")
        print(f"🔍 Searching competitors for '{keyword}'...")
        results = search_competitors(keyword)
        print(f"\nFound {len(results)} competitors:\n")
        for i, r in enumerate(results, 1):
            print(f"  {i}. [{r['domain']}] {r['title'][:60]}")
            print(f"     {r['url']}")

    elif command == "crawl":
        url = keyword or (sys.argv[2] if len(sys.argv) > 2 else "")
        if not url:
            print("ERROR: URL required")
            return
        print(f"🕷️ Crawling {url}...")
        data = crawl_website(url)
        print(json.dumps(data, indent=2, ensure_ascii=False))

    elif command == "analyze":
        url = keyword or (sys.argv[2] if len(sys.argv) > 2 else "")
        if not url:
            print("ERROR: URL required")
            return
        print(f"📊 Analyzing SEO for {url}...")
        crawl = crawl_website(url)
        analysis = analyze_seo_signals(crawl)
        print(json.dumps(analysis, indent=2, ensure_ascii=False))

    elif command == "social":
        name = keyword or (sys.argv[2] if len(sys.argv) > 2 else "")
        if not name:
            print("ERROR: Competitor name required")
            return
        print(f"📱 Checking social media for '{name}'...")
        result = search_social_media(name)
        print(json.dumps(result, indent=2, ensure_ascii=False))

    elif command == "report":
        keyword = keyword or (sys.argv[2] if len(sys.argv) > 2 else "travel")
        print(f"📄 Generating full report for '{keyword}'...")
        print("  Step 1: Searching competitors...")
        results = search_competitors(keyword, num_results=5)
        print(f"  Found {len(results)} competitors")

        print("  Step 2: Crawling websites...")
        crawled = []
        for r in results[:5]:
            print(f"    Crawling {r['domain']}...")
            data = crawl_website(r["url"])
            crawled.append(data)

        print("  Step 3: Analyzing SEO...")
        analyses = [analyze_seo_signals(c) for c in crawled]

        print("  Step 4: Generating report...")
        report = generate_competitor_report(keyword, crawled, analyses)
        print("\n" + report)


if __name__ == "__main__":
    main()


"""
SEO/GEO Engine — Phase 2 of website-generator skill.

Keyword expansion, topic clustering, difficulty assessment,
content calendar generation, and GEO optimization configuration.

No external API dependencies — uses heuristic-based analysis.
The orchestrating AI agent provides the real intelligence.

Dependencies: none (standard library only)
"""

import json
import re
import sys
from datetime import datetime
from typing import Optional


# ---------------------------------------------------------------------------
# 1. Keyword Expansion
# ---------------------------------------------------------------------------

def expand_keywords(
    seed_keyword: str,
    language: str = "en",
    target_markets: list[str] = None,
) -> list[dict]:
    """Expand a seed keyword with modifiers to generate long-tail variations.

    Uses pattern-based expansion. The agent should supplement with
    search-based data when available.

    Returns:
        List of {keyword, modifier_type, estimated_intent}
    """
    target_markets = target_markets or []
    words = seed_keyword.strip().lower()
    results = []

    # Informational modifiers
    info_mods = [
        ("guide", "informational"),
        ("tips", "informational"),
        ("how to", "informational"),
        ("what is", "informational"),
        ("where to", "informational"),
        ("when to", "informational"),
        ("why", "informational"),
        ("things to do", "informational"),
        ("itinerary", "informational"),
        ("blog", "informational"),
        ("photography spots", "informational"),
        ("history", "informational"),
        ("culture", "informational"),
        ("weather", "informational"),
        ("map", "informational"),
        ("facts", "informational"),
        ("FAQ", "informational"),
    ]

    # Commercial/transactional modifiers
    commercial_mods = [
        ("best", "commercial"),
        ("top", "commercial"),
        ("review", "commercial"),
        ("vs", "commercial"),
        ("comparison", "commercial"),
        ("alternative", "commercial"),
        ("affordable", "transactional"),
        ("cheap", "transactional"),
        ("luxury", "transactional"),
        ("premium", "transactional"),
        ("book", "transactional"),
        ("buy", "transactional"),
        ("price", "transactional"),
        ("cost", "transactional"),
        ("deal", "transactional"),
        ("discount", "transactional"),
    ]

    # Temporal modifiers
    current_year = datetime.now().year
    temporal_mods = [
        (str(current_year), "informational"),
        (f"{current_year} guide", "informational"),
        ("this year", "informational"),
        ("today", "informational"),
        ("seasonal", "informational"),
        ("winter", "informational"),
        ("summer", "informational"),
        ("spring", "informational"),
        ("autumn", "informational"),
    ]

    # Duration/itinerary modifiers (for travel)
    duration_mods = [
        ("1 day", "informational"),
        ("2 days", "informational"),
        ("3 days", "informational"),
        ("5 days", "informational"),
        ("7 days", "informational"),
        ("weekend", "informational"),
        ("2 weeks", "informational"),
    ]

    # Question formats
    question_mods = [
        (f"what to see in {words}", "informational"),
        (f"is {words} worth visiting", "informational"),
        (f"how to get to {words}", "informational"),
        (f"how much does {words} cost", "commercial"),
        (f"best time to visit {words}", "informational"),
        (f"where to stay in {words}", "commercial"),
        (f"what to eat in {words}", "informational"),
        (f"is {words} safe", "informational"),
    ]

    # Generate combinations
    for mod, intent in info_mods + commercial_mods:
        results.append({
            "keyword": f"{words} {mod}",
            "modifier_type": "prefix" if mod in ["how to", "what is", "where to", "when to", "why"] else "suffix",
            "estimated_intent": intent,
        })
        # Also try reversed order for some
        if intent == "informational" and mod not in ["how to", "what is", "where to", "when to", "why"]:
            results.append({
                "keyword": f"{mod} {words}",
                "modifier_type": "prefix",
                "estimated_intent": intent,
            })

    for mod, intent in temporal_mods:
        results.append({
            "keyword": f"{words} {mod}",
            "modifier_type": "temporal",
            "estimated_intent": intent,
        })

    for mod, intent in duration_mods:
        results.append({
            "keyword": f"{words} {mod}",
            "modifier_type": "duration",
            "estimated_intent": intent,
        })

    for question, intent in question_mods:
        results.append({
            "keyword": question,
            "modifier_type": "question",
            "estimated_intent": intent,
        })

    # Location modifiers
    for market in target_markets:
        results.append({
            "keyword": f"{words} {market.lower()}",
            "modifier_type": "geo",
            "estimated_intent": "navigational",
        })
        results.append({
            "keyword": f"{words} from {market.lower()}",
            "modifier_type": "geo",
            "estimated_intent": "informational",
        })

    # Deduplicate
    seen = set()
    unique = []
    for r in results:
        if r["keyword"] not in seen:
            seen.add(r["keyword"])
            unique.append(r)

    return unique


# ---------------------------------------------------------------------------
# 2. Keyword Clustering
# ---------------------------------------------------------------------------

def cluster_keywords(keywords: list[dict]) -> list[dict]:
    """Group keywords into topic clusters based on shared terms.

    Simple approach: group by modifier type and shared root words.

    Returns:
        List of {cluster_name, pillar_keyword, supporting_keywords, intent_mix}
    """
    clusters = {}

    # Group by modifier type first
    type_groups = {}
    for kw in keywords:
        mod_type = kw.get("modifier_type", "other")
        if mod_type not in type_groups:
            type_groups[mod_type] = []
        type_groups[mod_type].append(kw)

    # Map modifier types to cluster names
    cluster_names = {
        "suffix": "General Guides",
        "prefix": "How-To & Guides",
        "temporal": "Seasonal & Timely",
        "duration": "Itineraries & Planning",
        "question": "FAQ & Questions",
        "geo": "Location-Specific",
    }

    for mod_type, kw_list in type_groups.items():
        name = cluster_names.get(mod_type, f"Cluster: {mod_type}")

        # Find the shortest keyword as pillar (usually most general)
        sorted_kws = sorted(kw_list, key=lambda x: len(x["keyword"]))
        pillar = sorted_kws[0]["keyword"] if sorted_kws else ""

        # Intent distribution
        intents = [kw["estimated_intent"] for kw in kw_list]
        intent_counts = {}
        for i in intents:
            intent_counts[i] = intent_counts.get(i, 0) + 1

        clusters[mod_type] = {
            "cluster_name": name,
            "pillar_keyword": pillar,
            "supporting_keywords": [kw["keyword"] for kw in kw_list[1:]],
            "total_keywords": len(kw_list),
            "intent_mix": intent_counts,
        }

    return list(clusters.values())


# ---------------------------------------------------------------------------
# 3. Difficulty Assessment
# ---------------------------------------------------------------------------

def assess_difficulty(
    keyword: str,
    competitor_data: list[dict] = None,
) -> dict:
    """Estimate keyword difficulty using heuristics.

    Factors: word count, question format, commercial intent, specificity.

    Returns:
        {keyword, difficulty, priority, reasoning}
    """
    competitor_data = competitor_data or []
    word_count = len(keyword.split())
    reasoning = []

    # Base difficulty from word count (longer = usually easier)
    if word_count >= 5:
        base_score = 2  # Easy
        reasoning.append(f"Long-tail ({word_count} words) = lower competition")
    elif word_count >= 3:
        base_score = 5  # Medium
        reasoning.append(f"Medium-tail ({word_count} words)")
    else:
        base_score = 8  # Hard
        reasoning.append(f"Short-tail ({word_count} words) = higher competition")

    # Question format (usually easier)
    if any(keyword.lower().startswith(q) for q in ["how", "what", "where", "when", "why", "is", "can"]):
        base_score -= 2
        reasoning.append("Question format = featured snippet opportunity")

    # Commercial/transactional intent (usually harder)
    commercial_terms = ["buy", "price", "cost", "best", "top", "review", "cheap", "deal", "book"]
    if any(t in keyword.lower() for t in commercial_terms):
        base_score += 2
        reasoning.append("Commercial intent = more competition")

    # Temporal/year modifiers (moderate, but fresh opportunity)
    if any(str(y) in keyword for y in range(2024, 2028)):
        base_score -= 1
        reasoning.append("Year modifier = freshness opportunity")

    # Specificity (more specific = easier)
    specific_terms = ["weekend", "winter", "summer", "budget", "luxury", "family", "solo"]
    if any(t in keyword.lower() for t in specific_terms):
        base_score -= 1
        reasoning.append("Specific modifier = niche targeting")

    # Clamp score
    base_score = max(1, min(10, base_score))

    # Map to difficulty level
    if base_score <= 3:
        difficulty = "low"
        priority = 3  # ★★★ quick win
    elif base_score <= 6:
        difficulty = "medium"
        priority = 2  # ★★
    else:
        difficulty = "high"
        priority = 1  # ★

    return {
        "keyword": keyword,
        "difficulty": difficulty,
        "difficulty_score": base_score,
        "priority": priority,
        "priority_stars": "★" * priority,
        "reasoning": reasoning,
    }


# ---------------------------------------------------------------------------
# 4. Content Calendar
# ---------------------------------------------------------------------------

def generate_content_calendar(
    clusters: list[dict],
    months: int = 6,
) -> list[dict]:
    """Generate a month-by-month content publishing plan.

    Strategy:
    - Month 1-2: Foundation (pillar pages + easy wins)
    - Month 3-4: Growth (medium difficulty)
    - Month 5-6: Authority (harder keywords + link building)
    """
    calendar = []

    # Collect all keywords with difficulty
    all_keywords = []
    for cluster in clusters:
        pillar = {
            "keyword": cluster["pillar_keyword"],
            "type": "pillar",
            "cluster": cluster["cluster_name"],
        }
        assessed = assess_difficulty(cluster["pillar_keyword"])
        pillar.update(assessed)
        all_keywords.append(pillar)

        for kw in cluster.get("supporting_keywords", [])[:8]:
            entry = {
                "keyword": kw,
                "type": "supporting",
                "cluster": cluster["cluster_name"],
            }
            assessed = assess_difficulty(kw)
            entry.update(assessed)
            all_keywords.append(entry)

    # Sort by priority (3=easy first) then difficulty score
    all_keywords.sort(key=lambda x: (-x.get("priority", 0), x.get("difficulty_score", 5)))

    # Distribute across months
    items_per_month = max(3, len(all_keywords) // months)

    for month_num in range(1, months + 1):
        start_idx = (month_num - 1) * items_per_month
        end_idx = start_idx + items_per_month
        month_items = all_keywords[start_idx:end_idx]

        if not month_items:
            continue

        # Determine phase
        if month_num <= 2:
            phase = "Foundation"
            focus = "Pillar pages + low-difficulty quick wins"
        elif month_num <= 4:
            phase = "Growth"
            focus = "Supporting content + medium-difficulty targets"
        else:
            phase = "Authority"
            focus = "Harder keywords + link building + GEO optimization"

        content_items = []
        for item in month_items:
            # Determine content type and word count
            if item["type"] == "pillar":
                content_type = "pillar_page"
                word_count = 2500
            elif item.get("modifier_type") == "question":
                content_type = "faq_section"
                word_count = 800
            else:
                content_type = "blog_post"
                word_count = 1500

            content_items.append({
                "title": _keyword_to_title(item["keyword"]),
                "target_keyword": item["keyword"],
                "content_type": content_type,
                "word_count_target": word_count,
                "difficulty": item["difficulty"],
                "priority": item["priority_stars"],
                "cluster": item["cluster"],
            })

        calendar.append({
            "month": month_num,
            "phase": phase,
            "focus": focus,
            "content_items": content_items,
        })

    return calendar


def _keyword_to_title(keyword: str) -> str:
    """Convert a keyword into a natural article title."""
    kw = keyword.strip()
    # If it starts with a question word, capitalize and add ?
    question_words = ["how", "what", "where", "when", "why", "is", "can", "does", "do"]
    first_word = kw.split()[0].lower()
    if first_word in question_words:
        return kw.capitalize() + "?"
    # If it contains "guide", "tips", etc.
    if any(w in kw.lower() for w in ["guide", "tips", "review", "comparison"]):
        return kw.title() + ": Everything You Need to Know"
    # Default: title case
    return kw.title()


# ---------------------------------------------------------------------------
# 5. GEO Configuration
# ---------------------------------------------------------------------------

def generate_geo_config(
    keyword: str,
    site_type: str = "website",
    site_name: str = "My Website",
    site_url: str = "https://example.com",
) -> dict:
    """Generate GEO (Generative Engine Optimization) configuration.

    Includes llms.txt, Schema.org types, and structured data templates.
    """
    # llms.txt content
    llms_txt = f"""# {site_name}

> {site_name} is a comprehensive resource for {keyword}.

## About
This website provides detailed guides, tips, and resources about {keyword}.

## Key Pages
- Home: {site_url}
- Blog: {site_url}/blog
- About: {site_url}/about

## Contact
For inquiries, visit {site_url}/contact

## Content Categories
"""
    # Add category-specific sections
    categories = {
        "travel": ["Destinations", "Itineraries", "Activities", "Accommodation", "Culture"],
        "ecommerce": ["Products", "Categories", "Deals", "Reviews", "Shipping"],
        "saas": ["Features", "Pricing", "Documentation", "Blog", "Support"],
        "portfolio": ["Projects", "About", "Services", "Contact", "Blog"],
        "docs": ["Getting Started", "API Reference", "Guides", "Examples", "FAQ"],
    }

    for cat in categories.get(site_type, ["Home", "About", "Blog", "Contact"]):
        llms_txt += f"- {cat}: {site_url}/{cat.lower().replace(' ', '-')}\n"

    # Schema.org types
    schema_map = {
        "travel": ["TouristAttraction", "TravelAction", "Place", "LodgingBusiness"],
        "ecommerce": ["Product", "Offer", "Review", "Organization"],
        "saas": ["SoftwareApplication", "WebApplication", "Organization"],
        "portfolio": ["Person", "CreativeWork", "Organization"],
        "docs": ["TechArticle", "HowTo", "SoftwareSourceCode"],
    }
    base_schemas = ["WebSite", "Organization", "WebPage", "Article",
                    "BreadcrumbList", "FAQPage"]
    specific_schemas = schema_map.get(site_type, [])

    # Structured data templates
    templates = {
        "WebSite": {
            "@context": "https://schema.org",
            "@type": "WebSite",
            "name": site_name,
            "url": site_url,
            "potentialAction": {
                "@type": "SearchAction",
                "target": f"{site_url}/search?q={{search_term_string}}",
                "query-input": "required name=search_term_string",
            },
        },
        "Organization": {
            "@context": "https://schema.org",
            "@type": "Organization",
            "name": site_name,
            "url": site_url,
            "logo": f"{site_url}/logo.png",
        },
        "Article": {
            "@context": "https://schema.org",
            "@type": "Article",
            "headline": "{{title}}",
            "description": "{{description}}",
            "author": {"@type": "Person", "name": "{{author}}"},
            "datePublished": "{{date}}",
            "image": "{{image}}",
        },
        "BreadcrumbList": {
            "@context": "https://schema.org",
            "@type": "BreadcrumbList",
            "itemListElement": [
                {
                    "@type": "ListItem",
                    "position": 1,
                    "name": "Home",
                    "item": site_url,
                },
            ],
        },
        "FAQPage": {
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "mainEntity": [
                {
                    "@type": "Question",
                    "name": "{{question}}",
                    "acceptedAnswer": {
                        "@type": "Answer",
                        "text": "{{answer}}",
                    },
                },
            ],
        },
    }

    return {
        "llms_txt": llms_txt,
        "schema_types": base_schemas + specific_schemas,
        "structured_data_templates": templates,
    }


# ---------------------------------------------------------------------------
# 6. Multi-language Keywords
# ---------------------------------------------------------------------------

def generate_i18n_keywords(
    primary_keywords: list[str],
    target_language: str,
) -> list[dict]:
    """Generate placeholder entries for multi-language keyword research.

    CRITICAL: This does NOT translate keywords. It creates a structure
    for independent keyword research in the target language.
    """
    results = []
    for kw in primary_keywords:
        results.append({
            "original_keyword": kw,
            "target_language": target_language,
            "translated_keyword": f"[NEEDS INDEPENDENT RESEARCH — DO NOT DIRECTLY TRANSLATE]",
            "note": (
                "Keywords must be independently researched for each language. "
                "Direct translation loses search intent and local nuance. "
                "Use the agent's LLM capability to research native search behavior "
                f"for '{kw}' concepts in {target_language}."
            ),
        })
    return results


# ---------------------------------------------------------------------------
# 7. Automated Execution Plan (Hermes Automation)
# ---------------------------------------------------------------------------

def generate_automation_plan(cms_url: str = "http://localhost:8055", site_name: str = "My Website", domain: str = "example.com") -> str:
    """Generate Hermes agent automation instructions for continuous SEO operations."""
    return f"""## 🤖 Automated SEO Operations (Hermes)

As an autonomous agent, you can execute this plan continuously. Use the `/schedule` or `/goal` command to start the auto-pilot loop.

### 1. Content Auto-Pilot Workflow
**Trigger**: Weekly or Bi-weekly via `/schedule`
1. Fetch the next pending keyword from the Content Calendar (Month 1-6).
2. Use `geo-seo-claude` and `claude-seo` tools to research the keyword and build an outline.
3. Write a comprehensive, E-E-A-T compliant article (1500-2500 words).
4. Use the Directus REST API (`POST {cms_url}/items/articles`) to push the article directly to the CMS.
   - *Ensure status is set to 'published' and slug is properly formatted.*
5. Update `task.md` or the calendar tracking file to mark the keyword as completed.

### 2. Link Building & Outreach Automation
**Trigger**: After publishing a Pillar Page
1. Use `spider-rs` or Jina Reader to search for:
   - Unlinked brand mentions: `"{site_name}" -site:{domain}`
   - Competitor backlinks: Extract domains linking to top competitors for the pillar keyword.
2. Draft personalized outreach emails asking for backlinks or mentions.
3. Save drafts to a local `outreach/` folder for user review.

### 3. Social Promotion Automation
**Trigger**: Immediately after CMS publication
1. Use `marketingskills` methodology to slice the article into:
   - 1 Twitter/X Thread (3-5 tweets)
   - 1 LinkedIn post (professional tone)
2. Save to `social_queue.md` or post via webhooks (if configured).

"""


# ---------------------------------------------------------------------------
# 8. Full SEO Plan Generator
# ---------------------------------------------------------------------------

def generate_seo_plan(
    keyword: str,
    site_type: str = "website",
    languages: list[str] = None,
    target_markets: list[str] = None,
    site_name: str = "My Website",
    site_url: str = "https://example.com",
) -> str:
    """Generate a complete SEO/GEO plan as Markdown.

    Orchestrates all sub-functions into a comprehensive document.
    """
    languages = languages or ["en"]
    target_markets = target_markets or []

    # Step 1: Expand keywords
    keywords = expand_keywords(keyword, languages[0], target_markets)

    # Step 2: Cluster
    clusters = cluster_keywords(keywords)

    # Step 3: Content calendar
    calendar = generate_content_calendar(clusters)

    # Step 4: GEO config
    geo = generate_geo_config(keyword, site_type, site_name, site_url)

    # Build Markdown plan
    plan = f"""# 📊 SEO/GEO Plan: {keyword}

**Site Type**: {site_type}
**Languages**: {', '.join(languages)}
**Target Markets**: {', '.join(target_markets) or 'Global'}
**Generated**: {datetime.now().strftime('%Y-%m-%d')}

---

## Methodology

Based on three SEO skill frameworks:
- **geo-seo-claude**: GEO + traditional SEO, optimizing AI search engine citations
- **claude-seo**: E-E-A-T content quality + Schema structured data + semantic clustering
- **seomachine**: Keyword research → content creation → optimization → publishing pipeline

**Strategy**: Topic Cluster + Geo-targeting + Independent multilingual keyword systems

---

## Keyword Clusters ({len(clusters)} topic groups, {len(keywords)} total keywords)

"""

    for i, cluster in enumerate(clusters, 1):
        plan += f"### Cluster {i}: {cluster['cluster_name']}\n\n"
        plan += f"**Pillar**: {cluster['pillar_keyword']}\n"
        plan += f"**Keywords**: {cluster['total_keywords']}\n"
        plan += f"**Intent Mix**: {json.dumps(cluster['intent_mix'])}\n\n"

        plan += "| Keyword | Difficulty | Priority |\n"
        plan += "|---------|-----------|----------|\n"

        all_kws = [cluster["pillar_keyword"]] + cluster.get("supporting_keywords", [])[:8]
        for kw in all_kws:
            assessed = assess_difficulty(kw)
            plan += f"| {kw} | {assessed['difficulty']} | {assessed['priority_stars']} |\n"

        plan += "\n"

    # Content Calendar
    plan += f"""---

## 📅 Content Calendar ({len(calendar)} months)

"""

    for month in calendar:
        plan += f"### Month {month['month']}: {month['phase']}\n"
        plan += f"*Focus: {month['focus']}*\n\n"
        plan += "| Title | Keyword | Type | Words | Difficulty |\n"
        plan += "|-------|---------|------|-------|------------|\n"
        for item in month["content_items"][:6]:
            plan += (f"| {item['title'][:40]} | {item['target_keyword'][:30]} | "
                     f"{item['content_type']} | {item['word_count_target']} | "
                     f"{item['difficulty']} {item['priority']} |\n")
        plan += "\n"

    # GEO Configuration
    plan += f"""---

## 🤖 GEO (Generative Engine Optimization)

### llms.txt
```
{geo['llms_txt']}
```

### Schema.org Types to Implement
{chr(10).join(f'- `{s}`' for s in geo['schema_types'])}

"""

    # Multi-language
    if len(languages) > 1:
        plan += """---

## 🌍 Multi-Language Strategy

> [!IMPORTANT]
> Keywords must be **independently researched** for each language.
> Direct translation loses search intent and local nuance.

"""
        for lang in languages[1:]:
            i18n = generate_i18n_keywords(
                [cluster["pillar_keyword"] for cluster in clusters[:5]],
                lang,
            )
            plan += f"### {lang.upper()} Keywords (requires independent research)\n\n"
            plan += "| English Concept | Status |\n"
            plan += "|----------------|--------|\n"
            for entry in i18n:
                plan += f"| {entry['original_keyword']} | ⚠️ Needs native research |\n"
            plan += "\n"

    # Automation Plan
    import urllib.parse
    domain = urllib.parse.urlparse(site_url).netloc or "example.com"
    automation = generate_automation_plan(site_name=site_name, domain=domain)
    plan += f"""---

{automation}---

> This SEO plan was auto-generated as a starting framework.
> The orchestrating AI agent should refine it with real search data
> from geo-seo-claude, claude-seo, and seomachine skills.
"""

    return plan


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
  python seo_engine.py keywords "scottish highlands travel"
  python seo_engine.py cluster "scottish highlands travel"
  python seo_engine.py difficulty "scottish highlands travel guide"
  python seo_engine.py calendar "scottish highlands travel"
  python seo_engine.py geo "scottish highlands travel" --type travel
  python seo_engine.py plan "scottish highlands travel" --type travel --langs en,zh
        """)
        return

    command = sys.argv[1]

    if command == "keywords":
        seed = sys.argv[2] if len(sys.argv) > 2 else "travel"
        keywords = expand_keywords(seed)
        print(f"🔑 Expanded '{seed}' into {len(keywords)} keywords:\n")
        for kw in keywords[:30]:
            print(f"  [{kw['estimated_intent'][:5]}] {kw['keyword']}")

    elif command == "cluster":
        seed = sys.argv[2] if len(sys.argv) > 2 else "travel"
        keywords = expand_keywords(seed)
        clusters = cluster_keywords(keywords)
        print(f"📊 Clustered into {len(clusters)} topic groups:\n")
        for c in clusters:
            print(f"  📁 {c['cluster_name']} ({c['total_keywords']} keywords)")
            print(f"     Pillar: {c['pillar_keyword']}")

    elif command == "difficulty":
        kw = sys.argv[2] if len(sys.argv) > 2 else "travel"
        result = assess_difficulty(kw)
        print(json.dumps(result, indent=2))

    elif command == "calendar":
        seed = sys.argv[2] if len(sys.argv) > 2 else "travel"
        keywords = expand_keywords(seed)
        clusters = cluster_keywords(keywords)
        calendar = generate_content_calendar(clusters)
        for month in calendar:
            print(f"\n📅 Month {month['month']}: {month['phase']}")
            for item in month["content_items"][:5]:
                print(f"  - [{item['difficulty']}] {item['title'][:50]}")

    elif command == "geo":
        seed = sys.argv[2] if len(sys.argv) > 2 else "travel"
        site_type = "website"
        for i, arg in enumerate(sys.argv):
            if arg == "--type" and i + 1 < len(sys.argv):
                site_type = sys.argv[i + 1]
        config = generate_geo_config(seed, site_type)
        print("llms.txt:")
        print(config["llms_txt"])
        print("\nSchema types:", config["schema_types"])

    elif command == "plan":
        seed = sys.argv[2] if len(sys.argv) > 2 else "travel"
        site_type = "website"
        langs = ["en"]
        for i, arg in enumerate(sys.argv):
            if arg == "--type" and i + 1 < len(sys.argv):
                site_type = sys.argv[i + 1]
            if arg == "--langs" and i + 1 < len(sys.argv):
                langs = sys.argv[i + 1].split(",")
        plan = generate_seo_plan(seed, site_type, langs)
        print(plan)

    else:
        print(f"Unknown command: {command}")


if __name__ == "__main__":
    main()

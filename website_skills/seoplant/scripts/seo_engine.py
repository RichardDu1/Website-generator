"""
SEOplant SEO/GEO Engine — Phase 2 of the pipeline.

Keyword expansion, topic clustering, difficulty assessment,
content calendar generation, and GEO optimization configuration.

Two modes:
  - Heuristic (default): Pattern-based keyword expansion, no API needed
  - DataForSEO (enhanced): Real search volume, KD, CPC, competition data

To enable DataForSEO mode, set environment variables:
  DATAFORSEO_EMAIL=your@email.com
  DATAFORSEO_PASSWORD=your-api-password
Or pass a DataForSEOClient instance to the enhanced functions.

Dependencies: none (standard library only for heuristic mode)
              requests (for DataForSEO mode)
"""

import json
import os
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
# 1b. Keyword Expansion with DataForSEO (real data)
# ---------------------------------------------------------------------------

def _get_dfseo_client():
    """Lazy-load the DataForSEO client if credentials are available."""
    try:
        from dataforseo_client import DataForSEOClient
        client = DataForSEOClient()
        if client.is_configured:
            return client
    except ImportError:
        pass
    return None


def expand_keywords_with_data(
    seed_keyword: str,
    language: str = "en",
    target_markets: list[str] = None,
    dfseo_client=None,
) -> list[dict]:
    """Expand a seed keyword using DataForSEO real data + heuristic templates.

    Combines:
      - DataForSEO keyword suggestions (real search volume, CPC, KD, intent)
      - Heuristic modifier templates (question formats, year modifiers, etc.)
      - Location-specific expansions from target markets

    Falls back to pure heuristic if DataForSEO is unavailable.

    Returns:
        List of {keyword, search_volume, cpc, competition, keyword_difficulty,
                 estimated_intent, source (dataforseo/heuristic)}
    """
    target_markets = target_markets or []
    dfseo = dfseo_client or _get_dfseo_client()
    results = []
    seen = set()

    # --- Source 1: DataForSEO keyword suggestions (real data) ---
    if dfseo:
        try:
            suggestions = dfseo.keyword_suggestions(seed_keyword, limit=80)
            for s in suggestions:
                kw = s["keyword"].strip().lower()
                if kw not in seen and kw != seed_keyword.lower():
                    seen.add(kw)
                    results.append({
                        "keyword": kw,
                        "search_volume": s.get("search_volume", 0),
                        "cpc": s.get("cpc", 0),
                        "competition": s.get("competition", 0),
                        "competition_index": s.get("competition_index", 0),
                        "keyword_difficulty": s.get("keyword_difficulty", 0),
                        "search_intent": s.get("search_intent", "unknown"),
                        "source": "dataforseo",
                    })

            # Also get data for location-modified keywords
            for market in target_markets[:5]:
                geo_kw = f"{seed_keyword} {market.lower()}"
                volumes = dfseo.search_volume([geo_kw])
                if geo_kw.lower() in volumes:
                    v = volumes[geo_kw.lower()]
                    if v.get("search_volume", 0) > 0:
                        results.append({
                            "keyword": geo_kw,
                            "search_volume": v.get("search_volume", 0),
                            "cpc": v.get("cpc", 0),
                            "competition": v.get("competition", 0),
                            "keyword_difficulty": 0,
                            "search_intent": "navigational",
                            "source": "dataforseo",
                        })
        except Exception as e:
            print(f"  [WARN] DataForSEO keyword expansion failed: {e}")

    # --- Source 2: Heuristic templates (always runs, ensures coverage) ---
    heuristic = expand_keywords(seed_keyword, language, target_markets)
    for h in heuristic:
        kw = h["keyword"].strip().lower()
        if kw not in seen:
            seen.add(kw)
            results.append({
                "keyword": kw,
                "search_volume": 0,
                "cpc": 0,
                "competition": 0,
                "competition_index": 0,
                "keyword_difficulty": 0,
                "search_intent": h.get("estimated_intent", "unknown"),
                "source": "heuristic",
            })

    # Sort: DataForSEO data first (by volume), then heuristic alphabetically
    results.sort(key=lambda x: (-x["search_volume"], x["keyword"]))
    return results


def enrich_with_search_volume(
    keywords: list[dict],
    dfseo_client=None,
) -> list[dict]:
    """Batch-enrich keyword list with real search volume from DataForSEO.

    Modifies keywords in-place by adding search_volume, cpc, competition, etc.
    Falls back gracefully — heuristic keywords keep their existing estimates.
    """
    dfseo = dfseo_client or _get_dfseo_client()
    if not dfseo:
        return keywords

    # Collect keywords that need enrichment (source=heuristic or volume=0)
    to_enrich = [
        kw for kw in keywords
        if kw.get("source") == "heuristic" or kw.get("search_volume", 0) == 0
    ]
    if not to_enrich:
        return keywords

    try:
        kw_list = [k["keyword"] for k in to_enrich[:100]]  # Batch limit
        volumes = dfseo.search_volume(kw_list)
        if "error" in volumes:
            return keywords

        vol_map = {k: v for k, v in volumes.items()}

        for kw in keywords:
            key = kw["keyword"].strip().lower()
            if key in vol_map:
                v = vol_map[key]
                kw["search_volume"] = v.get("search_volume", kw.get("search_volume", 0))
                kw["cpc"] = v.get("cpc", kw.get("cpc", 0))
                kw["competition"] = v.get("competition", kw.get("competition", 0))
                kw["competition_index"] = v.get("competition_index", kw.get("competition_index", 0))
                kw["source"] = "dataforseo"
    except Exception as e:
        print(f"  [WARN] Search volume enrichment failed: {e}")

    return keywords


# ---------------------------------------------------------------------------
# 2. Keyword Clustering
# ---------------------------------------------------------------------------

def cluster_keywords(keywords: list[dict]) -> list[dict]:
    """Group keywords into topic clusters based on shared terms and intent.

    Preserves DataForSEO metadata (search_volume, keyword_difficulty, etc.)
    for downstream difficulty assessment.

    Returns:
        List of {cluster_name, pillar_keyword, supporting_keywords,
                 total_keywords, intent_mix}
        where supporting_keywords is list of {keyword, search_volume, kd, ...}
    """
    clusters = {}

    # Group by search intent first if DataForSEO data is available
    intent_groups = {}
    for kw in keywords:
        intent = kw.get("search_intent") or kw.get("estimated_intent") or kw.get("modifier_type", "other")
        if intent not in intent_groups:
            intent_groups[intent] = []
        intent_groups[intent].append(kw)

    # Map intents to cluster names
    cluster_names = {
        "informational": "Information & Guides",
        "commercial": "Product Reviews & Comparisons",
        "transactional": "Buying Guides & Deals",
        "navigational": "Brand & Destination Pages",
        "suffix": "General Guides",
        "prefix": "How-To & Guides",
        "temporal": "Seasonal & Timely",
        "duration": "Itineraries & Planning",
        "question": "FAQ & Questions",
        "geo": "Location-Specific",
    }

    for intent, kw_list in intent_groups.items():
        name = cluster_names.get(intent, f"Cluster: {intent}")

        # Sort by search volume (highest first) for pillar selection
        sorted_kws = sorted(kw_list, key=lambda x: x.get("search_volume") or 0, reverse=True)
        pillar = sorted_kws[0]["keyword"] if sorted_kws else ""

        # Intent distribution
        all_intents = [kw.get("search_intent", kw.get("estimated_intent", "unknown")) for kw in kw_list]
        intent_counts = {}
        for i in all_intents:
            intent_counts[i] = intent_counts.get(i, 0) + 1

        clusters[intent] = {
            "cluster_name": name,
            "pillar_keyword": pillar,
            "pillar_data": sorted_kws[0] if sorted_kws else {},
            "supporting_keywords": sorted_kws[1:],  # Full dicts with metadata
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
    real_kd: int = None,
    search_volume: int = None,
) -> dict:
    """Estimate keyword difficulty using heuristics, optionally enhanced with real KD.

    When real KD (0-100) is available from DataForSEO, it takes priority.
    Otherwise falls back to heuristic analysis based on word count, intent, etc.

    Args:
        keyword: The keyword to assess
        competitor_data: Optional competitor analysis data
        real_kd: Real keyword difficulty score from DataForSEO (0-100)
        search_volume: Real monthly search volume from DataForSEO

    Returns:
        {keyword, difficulty, difficulty_score, priority, priority_stars, reasoning}
    """
    competitor_data = competitor_data or []
    reasoning = []

    # --- Path A: Real DataForSEO KD available ---
    if real_kd is not None:
        base_score = max(1, min(10, round(real_kd / 10)))  # 0-100 → 1-10
        reasoning.append(f"DataForSEO keyword difficulty: {real_kd}/100")

        # Adjust for search volume (high volume = more competition in practice)
        if search_volume and search_volume > 10000:
            base_score = min(10, base_score + 1)
            reasoning.append(f"High search volume ({search_volume:,}/mo) = competitive")
        elif search_volume and search_volume < 100:
            base_score = max(1, base_score - 1)
            reasoning.append(f"Low search volume ({search_volume}/mo) = low competition")

        # Word count still matters as a signal
        word_count = len(keyword.split())
        if word_count >= 5:
            base_score = max(1, base_score - 1)
            reasoning.append(f"Long-tail bonus ({word_count} words)")
        elif word_count <= 2:
            base_score = min(10, base_score + 1)
            reasoning.append(f"Short-tail penalty ({word_count} words)")

        base_score = max(1, min(10, base_score))
    else:
        # --- Path B: Pure heuristic (original logic, no API) ---
        word_count = len(keyword.split())
        if word_count >= 5:
            base_score = 2
            reasoning.append(f"Long-tail ({word_count} words) = lower competition")
        elif word_count >= 3:
            base_score = 5
            reasoning.append(f"Medium-tail ({word_count} words)")
        else:
            base_score = 8
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
        pillar_data = cluster.get("pillar_data", {})
        pillar_kw = cluster["pillar_keyword"]
        pillar = {
            "keyword": pillar_kw,
            "type": "pillar",
            "cluster": cluster["cluster_name"],
            "search_volume": pillar_data.get("search_volume", 0),
            "keyword_data": pillar_data,
        }
        assessed = assess_difficulty(
            pillar_kw,
            real_kd=pillar_data.get("keyword_difficulty"),
            search_volume=pillar_data.get("search_volume"),
        )
        pillar.update(assessed)
        all_keywords.append(pillar)

        for kw_entry in cluster.get("supporting_keywords", [])[:8]:
            # Handle both old format (strings) and new format (dicts)
            if isinstance(kw_entry, dict):
                kw_str = kw_entry.get("keyword", "")
                kw_data = kw_entry
            else:
                kw_str = kw_entry
                kw_data = {}

            entry = {
                "keyword": kw_str,
                "type": "supporting",
                "cluster": cluster["cluster_name"],
                "search_volume": kw_data.get("search_volume", 0),
                "keyword_data": kw_data,
            }
            assessed = assess_difficulty(
                kw_str,
                real_kd=kw_data.get("keyword_difficulty"),
                search_volume=kw_data.get("search_volume"),
            )
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

    # Step 1: Expand keywords (with DataForSEO if available)
    keywords = expand_keywords_with_data(keyword, languages[0], target_markets)

    # Step 2: Enrich with real search volume
    keywords = enrich_with_search_volume(keywords)

    # Step 2: Cluster
    clusters = cluster_keywords(keywords)

    # Step 3: Content calendar
    calendar = generate_content_calendar(clusters)

    # Step 4: GEO config
    geo = generate_geo_config(keyword, site_type, site_name, site_url)

    # Build Markdown plan
    dfseo_mode = any(kw.get("source") == "dataforseo" for kw in keywords)
    data_source = "DataForSEO + Heuristic" if dfseo_mode else "Heuristic (no API)"

    plan = f"""# SEO/GEO Plan: {keyword}

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

        if dfseo_mode:
            plan += "| Keyword | Volume | KD | CPC | Priority |\n"
            plan += "|---------|--------|-----|-----|----------|\n"
        else:
            plan += "| Keyword | Difficulty | Priority |\n"
            plan += "|---------|-----------|----------|\n"

        pillar_data = cluster.get("pillar_data", {})
        pillar_assessed = assess_difficulty(
            cluster["pillar_keyword"],
            real_kd=pillar_data.get("keyword_difficulty"),
            search_volume=pillar_data.get("search_volume"),
        )
        if dfseo_mode:
            plan += (f"| {cluster['pillar_keyword'][:40]} "
                    f"| {pillar_data.get('search_volume', '-')} "
                    f"| {pillar_data.get('keyword_difficulty', '-')} "
                    f"| ${pillar_data.get('cpc') or 0:.2f} "
                    f"| {pillar_assessed['priority_stars']} |\n")
        else:
            plan += f"| {cluster['pillar_keyword'][:40]} | {pillar_assessed['difficulty']} | {pillar_assessed['priority_stars']} |\n"

        for kw_entry in cluster.get("supporting_keywords", [])[:8]:
            if isinstance(kw_entry, dict):
                kw_str = kw_entry.get("keyword", "")
                kw_kd = kw_entry.get("keyword_difficulty")
                kw_vol = kw_entry.get("search_volume", 0)
                kw_cpc = kw_entry.get("cpc") or 0
            else:
                kw_str = kw_entry
                kw_kd = None
                kw_vol = None
                kw_cpc = 0

            assessed = assess_difficulty(kw_str, real_kd=kw_kd, search_volume=kw_vol)
            if dfseo_mode:
                plan += (f"| {kw_str[:40]} "
                        f"| {kw_vol or '-'} "
                        f"| {kw_kd or '-'} "
                        f"| ${kw_cpc:.2f} "
                        f"| {assessed['priority_stars']} |\n")
            else:
                plan += f"| {kw_str[:40]} | {assessed['difficulty']} | {assessed['priority_stars']} |\n"

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

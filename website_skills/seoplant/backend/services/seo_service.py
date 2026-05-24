"""
SEOplant Backend — SEO pipeline orchestrator.
Wraps the 5 CLI modules for use as an API service.
"""
import json
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent.parent.parent / "scripts"
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))


def run_competitor_analysis(keyword: str, site_type: str = "website") -> dict:
    """Phase 1: Analyze competitors and return structured data."""
    from competitor_intel import search_competitors, crawl_website, analyze_seo_signals

    results = search_competitors(keyword, site_type, num_results=5)
    crawled = []
    for r in results[:5]:
        data = crawl_website(r["url"])
        crawled.append(data)

    analyses = [analyze_seo_signals(c) for c in crawled]

    return {
        "competitors": [
            {
                "domain": c.get("domain", ""),
                "title": c.get("title", ""),
                "word_count": c.get("word_count", 0),
                "tech_stack": c.get("tech_hints", []),
                "schema_types": c.get("schema_types", []),
                "seo_score": a.get("overall_score", 0),
            }
            for c, a in zip(crawled, analyses)
        ],
        "common_weaknesses": _collect_weaknesses(analyses),
    }


def run_seo_plan(keyword: str, site_type: str = "website", language: str = "en") -> dict:
    """Phase 2: Generate full SEO strategy."""
    from seo_engine import (
        expand_keywords_with_data,
        enrich_with_search_volume,
        cluster_keywords,
        generate_content_calendar,
        generate_geo_config,
    )

    keywords = expand_keywords_with_data(keyword, language)
    keywords = enrich_with_search_volume(keywords)
    clusters = cluster_keywords(keywords)
    calendar = generate_content_calendar(clusters)
    geo = generate_geo_config(keyword, site_type, f"{keyword.title()} Site", "https://example.com")

    return {
        "total_keywords": len(keywords),
        "clusters": [
            {
                "name": c["cluster_name"],
                "pillar": c["pillar_keyword"],
                "total_keywords": c["total_keywords"],
                "intent_mix": c["intent_mix"],
            }
            for c in clusters
        ],
        "calendar_months": len(calendar),
        "schema_types": geo.get("schema_types", []),
    }


def run_site_generation(
    keyword: str,
    site_name: str,
    site_type: str = "website",
    site_url: str = "https://example.com",
    output_dir: str = None,
) -> dict:
    """Phase 4: Generate Astro static site."""
    from site_builder import scaffold_project

    out = output_dir or f"./generated/{keyword.replace(' ', '-')}"
    result = scaffold_project(str(Path(out)), site_name, keyword, site_url)
    return result


def _collect_weaknesses(analyses: list[dict]) -> list[str]:
    from collections import Counter
    all_recs = []
    for a in analyses:
        all_recs.extend(a.get("recommendations", []))
    return [rec for rec, _ in Counter(all_recs).most_common(5)]

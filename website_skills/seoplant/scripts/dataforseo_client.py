"""
SEOplant DataForSEO Client — real SEO data layer.

Wraps DataForSEO API v3 for keyword research, SERP analysis,
competitor discovery, and LLM visibility tracking.

Usage:
    from dataforseo_client import DataForSEOClient

    client = DataForSEOClient("your@email.com", "your-api-password")
    volumes = client.search_volume(["best hiking boots", "hiking boots review"])
    serp = client.serp_organic("best hiking boots")
    related = client.keywords_for_keywords(["hiking boots"])

Dependencies: requests
"""

import base64
import json
import sys
from datetime import datetime
from typing import Optional

try:
    import requests
except ImportError:
    print("ERROR: 'requests' package required. Install: pip install requests")
    sys.exit(1)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
BASE_URL = "https://api.dataforseo.com/v3"
REQUEST_TIMEOUT = 60

# Common location codes
LOCATIONS = {
    "us": 2840,       # United States
    "uk": 2826,       # United Kingdom
    "ca": 2124,       # Canada
    "au": 2036,       # Australia
    "de": 2276,       # Germany
    "fr": 2250,       # France
    "jp": 2392,       # Japan
    "cn": 2156,       # China
    "in": 2356,       # India
    "br": 2076,       # Brazil
    "global": 2840,   # Default: US
}

# Language codes
LANGUAGES = {
    "en": "English",
    "zh": "Chinese",
    "es": "Spanish",
    "de": "German",
    "fr": "French",
    "ja": "Japanese",
    "pt": "Portuguese",
    "ru": "Russian",
    "ko": "Korean",
    "it": "Italian",
    "nl": "Dutch",
    "ar": "Arabic",
}


class DataForSEOClient:
    """Client for DataForSEO API v3.

    DataForSEO is the most cost-effective SEO data provider.
    No monthly subscription — pay per request with a $50 minimum deposit.

    https://dataforseo.com/api
    """

    def __init__(
        self,
        email: str = None,
        password: str = None,
        location_code: int = 2840,
        language_code: str = "en",
    ):
        """Initialize the DataForSEO client.

        Args:
            email: DataForSEO account email (or set DATAFORSEO_EMAIL env var)
            password: DataForSEO API password (or set DATAFORSEO_PASSWORD env var)
            location_code: Google location code (default 2840 = US)
            language_code: Google language code (default "en")
        """
        import os

        self.email = email or os.environ.get("DATAFORSEO_EMAIL", "")
        self.password = password or os.environ.get("DATAFORSEO_PASSWORD", "")
        self.location_code = location_code
        self.language_code = language_code
        self._auth = base64.b64encode(
            f"{self.email}:{self.password}".encode()
        ).decode()

    @property
    def is_configured(self) -> bool:
        """Check if API credentials are available."""
        return bool(self.email and self.password)

    # ------------------------------------------------------------------
    # Keyword Data API
    # ------------------------------------------------------------------

    def search_volume(
        self,
        keywords: list[str],
        location_code: int = None,
        language_code: str = None,
    ) -> dict:
        """Get search volume, CPC, and competition for keywords.

        Uses: POST /v3/keywords_data/google/search_volume/live

        Returns:
            {keyword: {search_volume, cpc, competition, low_top_of_page_bid,
                       high_top_of_page_bid, monthly_searches: [...]}}
        """
        if not self.is_configured:
            return {"error": "DataForSEO not configured. Set DATAFORSEO_EMAIL and DATAFORSEO_PASSWORD."}

        payload = [
            {
                "keywords": keywords,
                "location_code": location_code or self.location_code,
                "language_code": language_code or self.language_code,
            }
        ]

        result = self._post("/keywords_data/google/search_volume/live", payload)
        if "error" in result:
            return result

        tasks = result.get("tasks", [])
        if not tasks:
            return {"error": "No tasks returned"}

        # Parse results into a keyword → data map
        parsed = {}
        for task in tasks:
            for item in task.get("result", []):
                kw = item.get("keyword", "").lower()
                monthly_searches = item.get("monthly_searches", [])
                parsed[kw] = {
                    "keyword": item.get("keyword", kw),
                    "search_volume": item.get("search_volume", 0),
                    "cpc": item.get("cpc", 0),
                    "competition": item.get("competition", 0),
                    "competition_index": item.get("competition_index", 0),
                    "low_top_of_page_bid": item.get("low_top_of_page_bid", 0),
                    "high_top_of_page_bid": item.get("high_top_of_page_bid", 0),
                    "monthly_searches": [
                        {"year": m.get("year"), "month": m.get("month"),
                         "search_volume": m.get("search_volume", 0)}
                        for m in monthly_searches
                    ] if monthly_searches else [],
                }

        return parsed

    def keywords_for_keywords(
        self,
        keywords: list[str],
        location_code: int = None,
        language_code: str = None,
        include_serp_info: bool = False,
    ) -> dict:
        """Get keyword ideas and metrics for seed keywords.

        Uses: POST /v3/keywords_data/google/keywords_for_keywords/live

        Returns:
            {seed_keyword: [{keyword, search_volume, cpc, competition,
                             keyword_difficulty, search_intent, ...}]}
        """
        if not self.is_configured:
            return {"error": "DataForSEO not configured."}

        payload = [
            {
                "keywords": keywords,
                "location_code": location_code or self.location_code,
                "language_code": language_code or self.language_code,
                "include_serp_info": include_serp_info,
                "limit": 100,  # Max keyword suggestions per seed
            }
        ]

        result = self._post("/keywords_data/google/keywords_for_keywords/live", payload)
        if "error" in result:
            return result

        tasks = result.get("tasks", [])
        parsed = {}
        for task in tasks:
            for item in task.get("result", []):
                seed = item.get("seed_keyword", "")
                suggestions = []
                for kw in item.get("keywords", []):
                    intent_info = kw.get("search_intent_info", {})
                    suggestions.append({
                        "keyword": kw.get("keyword", ""),
                        "search_volume": kw.get("search_volume", 0),
                        "cpc": kw.get("cpc", 0),
                        "competition": kw.get("competition", 0),
                        "competition_index": kw.get("competition_index", 0),
                        "keyword_difficulty": kw.get("keyword_difficulty", 0),
                        "monthly_searches": kw.get("monthly_searches", []),
                        "search_intent": intent_info.get("main_intent", "unknown") if intent_info else "unknown",
                    })
                parsed[seed] = sorted(
                    suggestions,
                    key=lambda x: x["search_volume"],
                    reverse=True,
                )
        return parsed

    def keyword_suggestions(
        self,
        keyword: str,
        location_code: int = None,
        language_code: str = None,
        limit: int = 50,
    ) -> list[dict]:
        """Get keyword suggestions from Google Autocomplete / Related Searches.

        Uses: POST /v3/keywords_data/google/keywords_for_keywords/live
        (same endpoint, but returns related keyword ideas)

        Returns:
            List of {keyword, search_volume, cpc, competition, keyword_difficulty}
        """
        result = self.keywords_for_keywords(
            [keyword],
            location_code=location_code,
            language_code=language_code,
        )
        if "error" in result:
            return []
        return result.get(keyword, [])[:limit]

    # ------------------------------------------------------------------
    # SERP API
    # ------------------------------------------------------------------

    def serp_organic(
        self,
        keyword: str,
        location_code: int = None,
        language_code: str = None,
        depth: int = 20,
    ) -> dict:
        """Get organic Google SERP results for a keyword.

        Uses: POST /v3/serp/google/organic/live/advanced

        Returns:
            {keyword: {total_results, items: [{url, title, description,
              breadcrumb, highlighted, main_domain, relative_url, etv, ...}]}}
        """
        if not self.is_configured:
            return {"error": "DataForSEO not configured."}

        payload = [
            {
                "keyword": keyword,
                "location_code": location_code or self.location_code,
                "language_code": language_code or self.language_code,
                "depth": depth,
                "device": "desktop",
                "os": "windows",
            }
        ]

        result = self._post("/serp/google/organic/live/advanced", payload)
        if "error" in result:
            return result

        tasks = result.get("tasks", [])
        parsed = {}
        for task in tasks:
            for item in task.get("result", []):
                kw = item.get("keyword", keyword)
                serp_items = item.get("items", [])
                parsed[kw] = {
                    "total_results": item.get("se_results_count", 0),
                    "items": [
                        {
                            "position": r.get("rank_group", i + 1),
                            "url": r.get("url", ""),
                            "title": r.get("title", ""),
                            "description": r.get("description", ""),
                            "breadcrumb": r.get("breadcrumb", ""),
                            "domain": r.get("domain", ""),
                            "relative_url": r.get("relative_url", ""),
                            "etv": r.get("etv", 0),  # Estimated traffic volume
                            "highlighted": r.get("highlighted", []),
                            "is_paid": r.get("type", "") == "paid",
                            "is_featured_snippet": "featured_snippet" in str(r.get("type", "")),
                        }
                        for i, r in enumerate(serp_items)
                        if r.get("type") == "organic"
                    ],
                }
        return parsed

    def serp_ai_overview(self, keyword: str) -> dict:
        """Check if a keyword triggers Google AI Overview and extract its content."""
        payload = [
            {
                "keyword": keyword,
                "location_code": self.location_code,
                "language_code": self.language_code,
                "depth": 10,
            }
        ]
        result = self._post("/serp/google/organic/live/advanced", payload)
        if "error" in result:
            return result

        for task in result.get("tasks", []):
            for item in task.get("result", []):
                for r in item.get("items", []):
                    if r.get("type") == "ai_overview":
                        return {
                            "has_ai_overview": True,
                            "content": r.get("text", ""),
                            "source_links": r.get("items", []),
                        }
        return {"has_ai_overview": False}

    # ------------------------------------------------------------------
    # Domain / Competitor Analysis
    # ------------------------------------------------------------------

    def domain_keywords(
        self,
        domain: str,
        location_code: int = None,
        limit: int = 200,
    ) -> list[dict]:
        """Get keywords a domain ranks for in organic search.

        Uses: POST /v3/keywords_data/google/ads_search_volume/live
        (with keyword suggestions from the domain)

        Note: For full domain analysis, use the Domain Analytics API
        (requires separate DataForSEO subscription).
        """
        if not self.is_configured:
            return []

        payload = [
            {
                "target": domain,
                "location_code": location_code or self.location_code,
                "language_code": self.language_code,
                "limit": limit,
            }
        ]
        result = self._post("/domain_analytics/technologies/domains_by_technology/live", payload)
        if "error" in result:
            return []
        return result.get("tasks", [{}])[0].get("result", [])[:limit]

    def competitor_domains(
        self,
        domain: str,
        location_code: int = None,
    ) -> list[dict]:
        """Find domains competing with the given domain in organic search.

        Uses: POST /v3/dataforseo_labs/google/competitors_domain/live
        """
        if not self.is_configured:
            return []

        payload = [
            {
                "target": domain,
                "location_code": location_code or self.location_code,
                "language_code": self.language_code,
                "limit": 10,
            }
        ]
        result = self._post("/dataforseo_labs/google/competitors_domain/live", payload)
        if "error" in result:
            return []

        competitors = []
        for task in result.get("tasks", []):
            for item in task.get("result", []):
                for c in item.get("items", []):
                    competitors.append({
                        "domain": c.get("domain", ""),
                        "avg_position": c.get("avg_position", 0),
                        "intersections": c.get("intersections", 0),
                        "competition_level": c.get("competition_level", "unknown"),
                    })
        return competitors

    # ------------------------------------------------------------------
    # LLM / AI Visibility (GEO)
    # ------------------------------------------------------------------

    def llm_mentions(
        self,
        brand_name: str,
        llm: str = "all",
    ) -> dict:
        """Check brand mentions across AI/LLM platforms.

        Uses: POST /v3/dataforseo_labs/llm_mentions/live

        Args:
            brand_name: Your brand to check
            llm: 'chatgpt', 'gemini', 'perplexity', 'claude', or 'all'
        """
        if not self.is_configured:
            return {"error": "DataForSEO not configured."}

        payload = [{"brand_name": brand_name}]
        result = self._post("/dataforseo_labs/llm_mentions/live", payload)
        if "error" in result:
            return result

        mentions = {}
        for task in result.get("tasks", []):
            for item in task.get("result", []):
                llm_name = item.get("llm", "unknown")
                mentions[llm_name] = item.get("items", [])[:20]
        return mentions

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _post(self, endpoint: str, payload: list[dict]) -> dict:
        """Make a POST request to the DataForSEO API."""
        url = BASE_URL + endpoint
        headers = {
            "Authorization": f"Basic {self._auth}",
            "Content-Type": "application/json",
        }
        try:
            resp = requests.post(
                url,
                json=payload,
                headers=headers,
                timeout=REQUEST_TIMEOUT,
            )
            if resp.status_code == 200:
                data = resp.json()
                if data.get("status_code") and data["status_code"] >= 40000:
                    return {
                        "error": data.get("status_message", "API error"),
                        "code": data.get("status_code"),
                    }
                return data
            else:
                return {
                    "error": f"HTTP {resp.status_code}: {resp.text[:300]}",
                    "code": resp.status_code,
                }
        except requests.exceptions.Timeout:
            return {"error": "Request timed out"}
        except Exception as e:
            return {"error": str(e)}


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')

    if len(sys.argv) < 2:
        print("""
SEOplant DataForSEO Client

Commands:
  volume <kw1,kw2,...>        Bulk search volume lookup
  related <keyword>           Related keyword suggestions
  serp <keyword>              Google SERP analysis
  ai-overview <keyword>       Check Google AI Overview presence
  competitors <domain>        Find competing domains
  llm <brand>                 Check LLM mentions across AI platforms
  test                        Verify API connection

Usage:
  python dataforseo_client.py volume "hiking boots,trail shoes"
  python dataforseo_client.py related "best hiking boots"
  python dataforseo_client.py serp "scottish highlands travel"
  python dataforseo_client.py ai-overview "how to tie hiking boots"
  python dataforseo_client.py competitors "rei.com"
  python dataforseo_client.py llm "SEOplant"
  python dataforseo_client.py test

Credentials: Set DATAFORSEO_EMAIL and DATAFORSEO_PASSWORD environment variables,
or pass --email and --password flags.
        """)
        return

    command = sys.argv[1]

    # Parse optional credentials
    import os
    email = os.environ.get("DATAFORSEO_EMAIL", "")
    password = os.environ.get("DATAFORSEO_PASSWORD", "")
    loc = 2840

    for i, arg in enumerate(sys.argv):
        if arg == "--email" and i + 1 < len(sys.argv):
            email = sys.argv[i + 1]
        elif arg == "--password" and i + 1 < len(sys.argv):
            password = sys.argv[i + 1]
        elif arg == "--loc" and i + 1 < len(sys.argv):
            loc = int(sys.argv[i + 1])

    client = DataForSEOClient(email, password, loc)

    if command == "test":
        print("Testing DataForSEO connection...")
        if not client.is_configured:
            print("ERROR: Set DATAFORSEO_EMAIL and DATAFORSEO_PASSWORD environment variables.")
            return
        result = client.search_volume(["test keyword"])
        if "error" in result:
            print(f"Connection failed: {result['error']}")
        else:
            print("Connection OK — API credentials working.")

    elif command == "volume":
        kws = sys.argv[2].split(",") if len(sys.argv) > 2 else ["seo tools"]
        kws = [k.strip() for k in kws]
        print(f"Search volume for {len(kws)} keywords...")
        result = client.search_volume(kws)
        if "error" in result:
            print(f"Error: {result['error']}")
        else:
            for kw, data in result.items():
                print(f"  {data['keyword']}: {data['search_volume']:,}/mo | "
                      f"CPC ${data['cpc']:.2f} | Competition {data['competition']:.2f}")

    elif command == "related":
        kw = sys.argv[2] if len(sys.argv) > 2 else "seo tools"
        print(f"Related keywords for '{kw}'...")
        suggestions = client.keyword_suggestions(kw)
        for s in suggestions[:20]:
            print(f"  {s['keyword']}: {s['search_volume']:,}/mo | "
                  f"KD {s.get('keyword_difficulty', '?')} | "
                  f"Intent: {s.get('search_intent', '?')}")

    elif command == "serp":
        kw = sys.argv[2] if len(sys.argv) > 2 else "best seo tools"
        print(f"SERP for '{kw}'...")
        result = client.serp_organic(kw)
        if "error" in result:
            print(f"Error: {result['error']}")
        else:
            for kw, data in result.items():
                print(f"Total results: {data['total_results']:,}")
                for r in data["items"][:10]:
                    print(f"  #{r['position']} {r['domain']}")
                    print(f"     {r['title'][:80]}")

    elif command == "ai-overview":
        kw = sys.argv[2] if len(sys.argv) > 2 else "how to start a blog"
        print(f"AI Overview check for '{kw}'...")
        result = client.serp_ai_overview(kw)
        print(json.dumps(result, indent=2, ensure_ascii=False))

    elif command == "competitors":
        domain = sys.argv[2] if len(sys.argv) > 2 else "ahrefs.com"
        print(f"Competitors for {domain}...")
        result = client.competitor_domains(domain)
        for c in result:
            print(f"  {c['domain']}: {c['intersections']} shared keywords, "
                  f"avg pos {c['avg_position']}")

    elif command == "llm":
        brand = sys.argv[2] if len(sys.argv) > 2 else "SEOplant"
        print(f"LLM mentions for '{brand}'...")
        mentions = client.llm_mentions(brand)
        if "error" in mentions:
            print(f"Error: {mentions['error']}")
        else:
            for llm, items in mentions.items():
                print(f"  {llm}: {len(items)} mentions")

    else:
        print(f"Unknown command: {command}")


if __name__ == "__main__":
    main()

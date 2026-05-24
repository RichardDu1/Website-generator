"""
SEOplant Backend — FastAPI application configuration.
"""
import os
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
BACKEND_DIR = Path(__file__).resolve().parent

# Server
HOST = os.getenv("SEOPLANT_HOST", "0.0.0.0")
PORT = int(os.getenv("SEOPLANT_PORT", "8800"))
DEBUG = os.getenv("SEOPLANT_DEBUG", "true").lower() == "true"
SECRET_KEY = os.getenv("SEOPLANT_SECRET", "change-me-in-production-use-a-random-64-char-string")

# Database — SQLite for dev, PostgreSQL for prod
DATABASE_URL = os.getenv(
    "DATABASE_URL",
    f"sqlite:///{BACKEND_DIR / 'seoplant.db'}"
)

# JWT
JWT_ALGORITHM = "HS256"
JWT_EXPIRE_MINUTES = int(os.getenv("JWT_EXPIRE_MINUTES", "1440"))  # 24 hours

# Stripe
STRIPE_SECRET_KEY = os.getenv("STRIPE_SECRET_KEY", "")
STRIPE_WEBHOOK_SECRET = os.getenv("STRIPE_WEBHOOK_SECRET", "")
STRIPE_PRICE_PRO = os.getenv("STRIPE_PRICE_PRO", "price_pro_monthly")   # $79/mo
STRIPE_PRICE_AGENCY = os.getenv("STRIPE_PRICE_AGENCY", "price_agency_monthly")  # $199/mo

# DataForSEO
DATAFORSEO_EMAIL = os.getenv("DATAFORSEO_EMAIL", "")
DATAFORSEO_PASSWORD = os.getenv("DATAFORSEO_PASSWORD", "")

# Credit costs per operation
CREDIT_COSTS = {
    "competitor_analysis": 5,
    "seo_plan": 10,
    "site_generation": 20,
    "programmatic_page": 1,
    "deploy": 5,
    "keyword_research": 2,
}

# Free tier limits
FREE_TIER_CREDITS = 100
FREE_TIER_SITES = 1

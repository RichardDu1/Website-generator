"""
SEOplant Backend — FastAPI application entry point.
"""
from pathlib import Path

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse, RedirectResponse
from fastapi import Request

from .config import DEBUG, HOST, PORT, SECRET_KEY
from .database import init_db
from .routes import auth, projects, seo, billing

app = FastAPI(
    title="SEOplant API",
    version="0.1.0",
    description="AI Programmatic SEO Platform — Backend API",
)

# CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["https://seoplant.io", "http://localhost:4321", "http://localhost:8800"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Static files
_BACKEND = Path(__file__).resolve().parent
app.mount("/static", StaticFiles(directory=str(_BACKEND / "static")), name="static")

# API routes
app.include_router(auth.router)
app.include_router(projects.router)
app.include_router(seo.router)
app.include_router(billing.router)


@app.on_event("startup")
def on_startup():
    init_db()


# ── Dashboard pages (server-rendered HTML) ──

_DASHBOARD_TEMPLATES = _BACKEND / "templates"


def _render(template_name: str, **ctx) -> HTMLResponse:
    """Simple template renderer — reads HTML files and substitutes {{ vars }}."""
    path = _DASHBOARD_TEMPLATES / template_name
    if not path.exists():
        return HTMLResponse(f"<h1>Template not found: {template_name}</h1>", status_code=404)
    html = path.read_text(encoding="utf-8")
    for key, val in ctx.items():
        html = html.replace("{{ " + key + " }}", str(val) if val else "")
    return HTMLResponse(html)


@app.get("/dashboard", response_class=HTMLResponse)
def dashboard_page(request: Request):
    """Serve the main dashboard page (auth wall in JS)."""
    return _render("dashboard.html")


@app.get("/dashboard/login", response_class=HTMLResponse)
def login_page():
    return _render("login.html")


@app.get("/dashboard/register", response_class=HTMLResponse)
def register_page():
    return _render("register.html")


@app.get("/")
def root():
    return RedirectResponse("https://seoplant.io")


# ── Run ──
if __name__ == "__main__":
    import uvicorn
    print(f"Starting SEOplant backend on http://{HOST}:{PORT}")
    uvicorn.run("backend.main:app", host=HOST, port=PORT, reload=DEBUG)

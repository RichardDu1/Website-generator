"""
SEOplant Backend — SEO pipeline routes.
Wires the existing CLI modules as REST endpoints with credit tracking.
"""
import json
import sys
from pathlib import Path

from fastapi import APIRouter, Depends, HTTPException, BackgroundTasks
from sqlalchemy.orm import Session

from ..auth import get_current_user
from ..database import get_db
from ..models import User, Project
from ..config import CREDIT_COSTS
from ..schemas import SEOAnalyzeRequest, SEOGenerateRequest, DeployRequest, DeployResponse
from .projects import _get_owned_project, _deduct_credits

# Add parent scripts dir to path so we can import existing modules
_SCRIPTS = Path(__file__).resolve().parent.parent.parent / "scripts"
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

router = APIRouter(prefix="/api/seo", tags=["seo"])


@router.post("/analyze-competitors")
def analyze_competitors(
    body: SEOAnalyzeRequest,
    user: User = Depends(get_current_user),
    db: Session = Depends(get_db),
):
    """Run competitor intelligence on a keyword. Costs 5 credits."""
    cost = CREDIT_COSTS["competitor_analysis"]
    if not _deduct_credits(user, cost, "competitor_analysis", "", db):
        raise HTTPException(402, f"Not enough credits. Need {cost}, have {user.credits_remaining}")

    db.refresh(user)

    from competitor_intel import search_competitors, crawl_website, analyze_seo_signals, generate_competitor_report

    results = search_competitors(body.keyword, body.site_type, num_results=5)
    crawled = []
    for r in results[:5]:
        data = crawl_website(r["url"])
        crawled.append(data)

    analyses = [analyze_seo_signals(c) for c in crawled]
    report_md = generate_competitor_report(body.keyword, crawled, analyses)

    return {
        "report_markdown": report_md,
        "competitors_analyzed": len(crawled),
        "credits_spent": cost,
        "credits_remaining": user.credits_remaining,
    }


@router.post("/generate-plan")
def generate_seo_plan(
    body: SEOAnalyzeRequest,
    user: User = Depends(get_current_user),
    db: Session = Depends(get_db),
):
    """Generate a full SEO plan (topic clusters, content calendar, GEO config). Costs 10 credits."""
    cost = CREDIT_COSTS["seo_plan"]
    if not _deduct_credits(user, cost, "seo_plan", "", db):
        raise HTTPException(402, f"Not enough credits. Need {cost}, have {user.credits_remaining}")

    db.refresh(user)

    from seo_engine import generate_seo_plan as _gen_plan

    plan_md = _gen_plan(body.keyword, body.site_type, [body.language])

    return {
        "plan_markdown": plan_md,
        "credits_spent": cost,
        "credits_remaining": user.credits_remaining,
    }


@router.post("/generate-site")
def generate_site(
    body: SEOGenerateRequest,
    user: User = Depends(get_current_user),
    db: Session = Depends(get_db),
):
    """Generate an Astro static site from a keyword + SEO plan. Costs 20 credits."""
    cost = CREDIT_COSTS["site_generation"]
    if not _deduct_credits(user, cost, "site_generation", body.project_id, db):
        raise HTTPException(402, f"Not enough credits. Need {cost}, have {user.credits_remaining}")

    project = _get_owned_project(body.project_id, user, db)

    from site_builder import scaffold_project, generate_seo_head

    sites_dir = Path("generated_sites")
    sites_dir.mkdir(exist_ok=True)
    site_dir = sites_dir / body.project_id

    result = scaffold_project(
        str(site_dir),
        site_name=body.site_name or f"{body.keyword.title()} Site",
        keyword=body.keyword,
        site_url=project.target_url or "https://example.com",
    )

    # Save result paths to project
    project.site_dir = str(site_dir)
    project.status = "generated"
    project.credits_spent = (project.credits_spent or 0) + cost
    db.commit()
    db.refresh(user)

    return {
        "site_dir": str(site_dir),
        "files_created": result.get("created_files", []),
        "credits_spent": cost,
        "credits_remaining": user.credits_remaining,
    }


@router.post("/deploy")
def deploy_site(
    body: DeployRequest,
    background_tasks: BackgroundTasks,
    user: User = Depends(get_current_user),
    db: Session = Depends(get_db),
):
    """Deploy a generated site to VPS, Vercel, or Cloudflare. Costs 5 credits."""
    cost = CREDIT_COSTS["deploy"]
    if not _deduct_credits(user, cost, "deploy", body.project_id, db):
        raise HTTPException(402, f"Not enough credits. Need {cost}, have {user.credits_remaining}")

    project = _get_owned_project(body.project_id, user, db)

    # Create deployment record
    from ..models import Deployment

    deploy = Deployment(
        project_id=body.project_id,
        target=body.target,
        domain=body.domain,
        server_host=body.server_host,
        status="deploying",
    )
    db.add(deploy)
    db.commit()
    db.refresh(deploy)

    # Queue actual deployment as background task
    background_tasks.add_task(
        _run_deployment,
        deploy.id,
        body.model_dump(),
        project.site_dir,
    )

    return {
        "deployment": DeployResponse.model_validate(deploy),
        "credits_spent": cost,
        "credits_remaining": user.credits_remaining - cost,
    }


def _run_deployment(deployment_id: str, deploy_args: dict, site_dir: str):
    """Background task: execute deployment."""
    # Reopen a fresh DB session for background work
    from ..database import SessionLocal
    from ..models import Deployment as DepModel

    db2 = SessionLocal()
    try:
        deploy = db2.query(DepModel).filter(DepModel.id == deployment_id).first()
        if not deploy:
            return

        target = deploy_args.get("target", "vps")

        if target == "cloudflare":
            _deploy_cloudflare(deploy, deploy_args, site_dir, db2)
        elif target == "vercel":
            _deploy_vercel(deploy, deploy_args, site_dir, db2)
        else:  # vps
            _deploy_vps(deploy, deploy_args, site_dir, db2)

    except Exception as e:
        if deploy:
            deploy.status = "failed"
            deploy.log_output = str(e)
            db2.commit()
    finally:
        db2.close()


def _deploy_cloudflare(deploy, args, site_dir, db):
    """Deploy to Cloudflare Pages via Wrangler."""
    import subprocess

    result = subprocess.run(
        ["npx", "wrangler", "pages", "deploy", site_dir, "--project-name", "seoplant-site"],
        capture_output=True, text=True, timeout=120,
    )
    deploy.log_output = result.stdout + "\n" + result.stderr
    deploy.status = "live" if result.returncode == 0 else "failed"
    db.commit()


def _deploy_vercel(deploy, args, site_dir, db):
    """Deploy to Vercel via CLI."""
    import subprocess

    result = subprocess.run(
        ["npx", "vercel", site_dir, "--prod", "--confirm"],
        capture_output=True, text=True, timeout=120,
    )
    deploy.log_output = result.stdout + "\n" + result.stderr
    deploy.status = "live" if result.returncode == 0 else "failed"
    db.commit()


def _deploy_vps(deploy, args, site_dir, db):
    """Deploy to customer VPS via SSH."""
    from deployer import VPSDeployer

    try:
        d = VPSDeployer(
            host=args.get("server_host", ""),
            user=args.get("server_user", "root"),
            port=args.get("server_port", 22),
        )
        result = d.deploy(
            domain=args.get("domain", ""),
            project_dir=site_dir,
            web_server="caddy",
        )
        deploy.log_output = json.dumps(result.get("actions", []), indent=2)
        deploy.status = "live" if result.get("verified") else "failed"
    except Exception as e:
        deploy.log_output = str(e)
        deploy.status = "failed"
    db.commit()

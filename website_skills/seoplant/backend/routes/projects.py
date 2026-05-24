"""
SEOplant Backend — Project CRUD routes.
"""
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session

from ..auth import get_current_user
from ..database import get_db
from ..models import User, Project, CreditTransaction
from ..config import CREDIT_COSTS
from ..schemas import ProjectCreate, ProjectUpdate, ProjectResponse

router = APIRouter(prefix="/api/projects", tags=["projects"])


@router.get("/", response_model=list[ProjectResponse])
def list_projects(user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    """List all projects for the current user."""
    return [
        ProjectResponse.model_validate(p)
        for p in db.query(Project).filter(Project.owner_id == user.id).order_by(Project.created_at.desc()).all()
    ]


@router.post("/", response_model=ProjectResponse, status_code=201)
def create_project(body: ProjectCreate, user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    """Create a new SEO project."""
    name = body.name or f"{body.keyword.title()} — {body.site_type.title()} Site"
    project = Project(
        owner_id=user.id,
        name=name,
        keyword=body.keyword,
        site_type=body.site_type,
    )
    db.add(project)
    db.commit()
    db.refresh(project)
    return ProjectResponse.model_validate(project)


@router.get("/{project_id}", response_model=ProjectResponse)
def get_project(project_id: str, user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    """Get a single project by ID."""
    project = _get_owned_project(project_id, user, db)
    return ProjectResponse.model_validate(project)


@router.patch("/{project_id}", response_model=ProjectResponse)
def update_project(project_id: str, body: ProjectUpdate, user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    """Update project fields."""
    project = _get_owned_project(project_id, user, db)
    for key, val in body.model_dump(exclude_unset=True).items():
        setattr(project, key, val)
    db.commit()
    db.refresh(project)
    return ProjectResponse.model_validate(project)


@router.delete("/{project_id}")
def delete_project(project_id: str, user: User = Depends(get_current_user), db: Session = Depends(get_db)):
    """Delete a project and its deployments."""
    project = _get_owned_project(project_id, user, db)
    db.delete(project)
    db.commit()
    return {"status": "deleted"}


def _get_owned_project(project_id: str, user: User, db: Session) -> Project:
    """Fetch a project, ensuring it belongs to the current user."""
    project = db.query(Project).filter(Project.id == project_id).first()
    if not project:
        raise HTTPException(404, "Project not found")
    if project.owner_id != user.id:
        raise HTTPException(403, "Not your project")
    return project


def _deduct_credits(user: User, amount: int, operation: str, project_id: str, db: Session) -> bool:
    """Deduct credits from user. Returns True if sufficient credits."""
    if user.credits_remaining < amount:
        return False
    user.credits_remaining -= amount
    user.credits_used_total += amount
    db.add(CreditTransaction(user_id=user.id, amount=-amount, operation=operation, project_id=project_id))
    db.commit()
    return True

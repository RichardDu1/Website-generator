"""
SEOplant Backend — SQLAlchemy models.
"""
import uuid
from datetime import datetime, timezone

from sqlalchemy import Column, String, Integer, Boolean, DateTime, ForeignKey, Text, Float
from sqlalchemy.orm import relationship

from .database import Base


def _new_id() -> str:
    return uuid.uuid4().hex[:12]


def _utcnow() -> datetime:
    return datetime.now(timezone.utc)


class User(Base):
    __tablename__ = "users"

    id = Column(String, primary_key=True, default=_new_id)
    email = Column(String, unique=True, nullable=False, index=True)
    hashed_password = Column(String, nullable=False)
    display_name = Column(String, default="")
    is_active = Column(Boolean, default=True)
    plan = Column(String, default="free")         # free / pro / agency / enterprise
    stripe_customer_id = Column(String, default="")
    stripe_subscription_id = Column(String, default="")
    credits_remaining = Column(Integer, default=100)  # Free tier: 100 credits
    credits_used_total = Column(Integer, default=0)
    created_at = Column(DateTime, default=_utcnow)
    updated_at = Column(DateTime, default=_utcnow, onupdate=_utcnow)

    projects = relationship("Project", back_populates="owner", order_by="Project.created_at.desc()")


class Project(Base):
    __tablename__ = "projects"

    id = Column(String, primary_key=True, default=_new_id)
    owner_id = Column(String, ForeignKey("users.id"), nullable=False, index=True)
    name = Column(String, nullable=False)
    keyword = Column(String, nullable=False)
    site_type = Column(String, default="website")  # website / ecommerce / travel / saas / docs
    status = Column(String, default="draft")       # draft / analyzing / generating / deploying / live / error
    target_url = Column(String, default="")
    site_dir = Column(String, default="")          # Path to generated Astro project
    seo_plan_json = Column(Text, default="")       # JSON blob of the SEO plan
    competitor_report = Column(Text, default="")    # JSON blob of competitor analysis
    credits_spent = Column(Integer, default=0)
    created_at = Column(DateTime, default=_utcnow)
    updated_at = Column(DateTime, default=_utcnow, onupdate=_utcnow)

    owner = relationship("User", back_populates="projects")
    deployments = relationship("Deployment", back_populates="project", order_by="Deployment.created_at.desc()")


class Deployment(Base):
    __tablename__ = "deployments"

    id = Column(String, primary_key=True, default=_new_id)
    project_id = Column(String, ForeignKey("projects.id"), nullable=False, index=True)
    target = Column(String, default="vps")         # vps / vercel / cloudflare
    domain = Column(String, default="")
    server_host = Column(String, default="")
    status = Column(String, default="pending")     # pending / deploying / live / failed
    log_output = Column(Text, default="")
    created_at = Column(DateTime, default=_utcnow)

    project = relationship("Project", back_populates="deployments")


class CreditTransaction(Base):
    __tablename__ = "credit_transactions"

    id = Column(String, primary_key=True, default=_new_id)
    user_id = Column(String, ForeignKey("users.id"), nullable=False, index=True)
    amount = Column(Integer, nullable=False)  # Negative = spent, Positive = purchased
    operation = Column(String, default="")    # "competitor_analysis", "purchase_1000", etc.
    project_id = Column(String, default="")
    created_at = Column(DateTime, default=_utcnow)
